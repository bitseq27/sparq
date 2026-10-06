//! The frozen `data-record` vocabulary, mirrored purely (WO-020 plan D13; see the crate docs on
//! why this is the *third* mirror and not redundancy).
//!
//! Field-for-field with `docs/api/instrument-wit/wit/types.wit` (the guest face) and with
//! `crates/sparq-streams/src/record.rs` (the broker's host-side native mirror). The wasm glue maps
//! the WIT `data-record` into [`DataRecord`]; the harness maps `sparq_streams::record::DataRecord`
//! into it. Both mappings are field-for-field and drift-tested, so the channel ORDER below is the
//! contract, never a convenience — a guest that misread it would draw the wrong feed silently.
//!
//! # Stamping (ADR-007, sources.wit decision C)
//!
//! On the wire every stamp is host-owned: the broker's `t_wall_ns`/`seq` are ADVISORY and the host
//! re-stamps both when it serves a window. The Observatory therefore never reads `t_wall_ns` as a
//! clock — it renders the record's own UTC time (a `t_utc` channel inside the schema) as absolute
//! UTC (plan D8: the guest has no wall clock), and shows "age" from the record's host-computed
//! `age_s` channel. Nothing here calls a clock; determinism is the whole point (D11 firewall).

/// One typed channel value — the frozen `data-value` variant, 1:1 with `types.wit`. The carriers
/// are closed; a value that does not fit one of these nine is not a record the core can read, and
/// the renderer refuses it in words rather than guessing a default.
#[derive(Clone, Debug, PartialEq)]
pub enum DataValue {
    /// dtype `f32`.
    Single(f32),
    /// dtype `f64` — the observatory's default measurement carrier.
    Double(f64),
    /// dtype `i32`.
    Int32(i32),
    /// dtype `i64` — unix seconds ride here.
    Int64(i64),
    /// dtype `bool`.
    Boolean(bool),
    /// dtype `vecN` — the f64 carrier (grid rows); N fixed per channel by the schema.
    Vec(Vec<f64>),
    /// dtype `enum` — an index into the schema's declared options (severity, visibility).
    Enumerated(u32),
    /// dtype `str` — a headline, a place, a raw feed line.
    Text(String),
    /// dtype `blob` — opaque bytes (TLE blocks ride here when SGP4 lands, INC6).
    Blob(Vec<u8>),
}

impl DataValue {
    /// The value as an `f64` for the numeric carriers only (`Single` widens; `Double`/`Int32`/
    /// `Int64` widen). Text/enum/vec/blob/bool answer `None` — a silent 0.0 for a non-number is the
    /// wrong-data-with-no-symptom bug the contract forbids.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Single(x) => Some(f64::from(*x)),
            Self::Double(x) => Some(*x),
            Self::Int32(n) => Some(f64::from(*n)),
            Self::Int64(n) => Some(*n as f64),
            _ => None,
        }
    }

    /// The value as a string slice, for the `Text` carrier only.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// The value as a `u32` enum index, for the `Enumerated` carrier only.
    #[must_use]
    pub fn as_enum(&self) -> Option<u32> {
        match self {
            Self::Enumerated(n) => Some(*n),
            _ => None,
        }
    }

    /// The value as an f64 slice, for the `Vec` carrier only.
    #[must_use]
    pub fn as_vec(&self) -> Option<&[f64]> {
        match self {
            Self::Vec(v) => Some(v.as_slice()),
            _ => None,
        }
    }
}

/// Record quality flags — the frozen `data-flags` enum, 1:1 with `types.wit`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DataFlags {
    /// Fresh and complete.
    #[default]
    Ok,
    /// Older than the stream's stale horizon (the cell LED reads STALE).
    Stale,
    /// A gap in the sequence.
    Discontinuity,
    /// Interpolated/modelled, not directly observed.
    Estimated,
}

/// One typed data record — the frozen `data-record`, 1:1 with `types.wit`.
#[derive(Clone, Debug, PartialEq)]
pub struct DataRecord {
    /// Schema identity, e.g. `observatory/timeseries` (the `@version` rides `schema_version`).
    pub schema_id: String,
    /// Schema version, e.g. `1`.
    pub schema_version: u32,
    /// The channel values, in the schema's declared order.
    pub channels: Vec<DataValue>,
    /// Host-owned wall stamp, nanoseconds. ADVISORY on the wire; the Observatory renders the
    /// record's own `t_utc` channel instead, never this (D8: no guest wall clock).
    pub t_wall_ns: u64,
    /// Audio-time sample, present only for audio-clock schemas. The observatory's schemas are all
    /// wall-clock, so this is `None` on every record the broker produces.
    pub t_sample: Option<u64>,
    /// Host-owned sequence number. ADVISORY on the wire.
    pub seq: u64,
    /// Quality/freshness flags.
    pub record_flags: DataFlags,
}

// --- the five observatory schema ids (mirror of sparq-streams/src/schema.rs) --------------------

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

/// Severity tier 0: unknown/unclassified.
pub const SEVERITY_UNKNOWN: u32 = 0;
/// Severity tier 1: low/informational.
pub const SEVERITY_LOW: u32 = 1;
/// Severity tier 2: moderate.
pub const SEVERITY_MODERATE: u32 = 2;
/// Severity tier 3: high.
pub const SEVERITY_HIGH: u32 = 3;
/// Severity tier 4: severe/extreme.
pub const SEVERITY_SEVERE: u32 = 4;

/// The severity tier vocabulary, index order — the `enumerated` options for the events schema. A
/// renderer that needs the labels reads this, never hard-codes them (drift-tested vs the broker's
/// `sparq_streams::schema::SEVERITY_OPTIONS`).
pub const SEVERITY_OPTIONS: [&str; 5] = ["unknown", "low", "moderate", "high", "severe"];

// --- the shared visibility scale (position schema, `visibility` channel) ------------------------

/// Visibility 0: unknown.
pub const VISIBILITY_UNKNOWN: u32 = 0;
/// Visibility 1: daylight (sunlit).
pub const VISIBILITY_DAYLIGHT: u32 = 1;
/// Visibility 2: eclipsed.
pub const VISIBILITY_ECLIPSED: u32 = 2;
/// Visibility 3: night.
pub const VISIBILITY_NIGHT: u32 = 3;

/// The visibility vocabulary, index order — the `enumerated` options for the position schema.
pub const VISIBILITY_OPTIONS: [&str; 4] = ["unknown", "daylight", "eclipsed", "night"];

impl DataRecord {
    /// The channel at `i`, or `None` if the record is shorter than the schema expects. A short
    /// record is a producer bug the renderer must not paper over with a default.
    #[must_use]
    pub fn channel(&self, i: usize) -> Option<&DataValue> {
        self.channels.get(i)
    }

    /// The channel at `i` as an `f64`, or `None` (absent or a non-number).
    #[must_use]
    pub fn num(&self, i: usize) -> Option<f64> {
        self.channel(i).and_then(DataValue::as_f64)
    }

    /// The channel at `i` as a string, or `None`.
    #[must_use]
    pub fn text(&self, i: usize) -> Option<&str> {
        self.channel(i).and_then(DataValue::as_str)
    }

    /// Whether this record's schema is one of the five observatory shapes.
    #[must_use]
    pub fn is_observatory(&self) -> bool {
        matches!(
            self.schema_id.as_str(),
            TIMESERIES_ID | EVENTS_ID | GRID_ID | POSITION_ID | TEXT_ID
        )
    }
}

// --- typed channel-order accessors (the schema, as code the renderers trust) --------------------
//
// Each accessor names the channel indices the schema doc fixes. They return Option so a malformed
// record (wrong carrier, too short) reads as "cannot draw this" rather than a silent 0.0.

/// `observatory/timeseries@1` — `{t_utc: i64, value: f64, age_s: f64}`.
#[must_use]
pub fn ts_channels(r: &DataRecord) -> Option<(i64, f64, f64)> {
    if r.schema_id != TIMESERIES_ID {
        return None;
    }
    let t = r.channel(0)?.as_f64()? as i64;
    let v = r.num(1)?;
    let age = r.num(2)?;
    Some((t, v, age))
}

/// `observatory/events@1` — `{t_utc: i64, lat: f64, lon: f64, magnitude: f64, depth_km: f64,
/// severity: enum, headline: str, age_s: f64}`. lat/lon are NaN for a non-geolocated event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EventChannels {
    /// Event time, unix seconds.
    pub t_utc: i64,
    /// Latitude, degrees (NaN = no position).
    pub lat: f64,
    /// Longitude, degrees (NaN = no position).
    pub lon: f64,
    /// Magnitude (stream-specific scale).
    pub magnitude: f64,
    /// Depth, km (0 where the stream has none).
    pub depth_km: f64,
    /// Severity tier index (see `SEVERITY_*`).
    pub severity: u32,
    /// Record age, seconds (host-computed, D8).
    pub age_s: f64,
}

/// Reads an events record's channels (headline is fetched separately via [`DataRecord::text`], so
/// the struct stays `Copy`).
#[must_use]
pub fn event_channels(r: &DataRecord) -> Option<EventChannels> {
    if r.schema_id != EVENTS_ID {
        return None;
    }
    Some(EventChannels {
        t_utc: r.channel(0)?.as_f64()? as i64,
        lat: r.num(1)?,
        lon: r.num(2)?,
        magnitude: r.num(3)?,
        depth_km: r.num(4)?,
        severity: r.channel(5)?.as_enum()?,
        age_s: r.num(7)?,
    })
}

/// The events record's headline (channel 6, the `str` carrier), or `None`.
#[must_use]
pub fn event_headline(r: &DataRecord) -> Option<&str> {
    if r.schema_id == EVENTS_ID {
        r.text(6)
    } else {
        None
    }
}

/// `observatory/grid@1` — `{cols: i32, rows: i32, lat0: f64, lon0: f64, step_deg: f64,
/// values: vecN, t_utc: i64, age_s: f64}`. `values` is row-major, NaN = no cell.
#[derive(Clone, Debug, PartialEq)]
pub struct GridChannels<'a> {
    /// Column count.
    pub cols: i32,
    /// Row count.
    pub rows: i32,
    /// Latitude of the grid origin (cell 0,0 centre), degrees.
    pub lat0: f64,
    /// Longitude of the grid origin, degrees.
    pub lon0: f64,
    /// Cell size, degrees.
    pub step_deg: f64,
    /// Row-major values, `rows * cols`; NaN = no cell.
    pub values: &'a [f64],
    /// Grid time, unix seconds.
    pub t_utc: i64,
    /// Record age, seconds.
    pub age_s: f64,
}

/// Reads a grid record's channels (borrows the `values` slice).
#[must_use]
pub fn grid_channels(r: &DataRecord) -> Option<GridChannels<'_>> {
    if r.schema_id != GRID_ID {
        return None;
    }
    Some(GridChannels {
        cols: r.channel(0)?.as_f64()? as i32,
        rows: r.channel(1)?.as_f64()? as i32,
        lat0: r.num(2)?,
        lon0: r.num(3)?,
        step_deg: r.num(4)?,
        values: r.channel(5)?.as_vec()?,
        t_utc: r.channel(6)?.as_f64()? as i64,
        age_s: r.num(7)?,
    })
}

/// `observatory/position@1` — `{lat: f64, lon: f64, alt_km: f64, vel_kms: f64, footprint_km: f64,
/// visibility: enum, t_utc: i64, age_s: f64}`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PositionChannels {
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
    /// Visibility tier index (see `VISIBILITY_*`).
    pub visibility: u32,
    /// Position time, unix seconds.
    pub t_utc: i64,
    /// Record age, seconds.
    pub age_s: f64,
}

/// Reads a position record's channels.
#[must_use]
pub fn position_channels(r: &DataRecord) -> Option<PositionChannels> {
    if r.schema_id != POSITION_ID {
        return None;
    }
    Some(PositionChannels {
        lat: r.num(0)?,
        lon: r.num(1)?,
        alt_km: r.num(2)?,
        vel_kms: r.num(3)?,
        footprint_km: r.num(4)?,
        visibility: r.channel(5)?.as_enum()?,
        t_utc: r.channel(6)?.as_f64()? as i64,
        age_s: r.num(7)?,
    })
}

/// `observatory/text@1` — `{text: str, issued_utc: i64, age_s: f64}`. `text` is the raw payload,
/// verbatim (the operator asked for the raw stream literally, plan D8).
#[must_use]
pub fn text_channels(r: &DataRecord) -> Option<(&str, i64, f64)> {
    if r.schema_id != TEXT_ID {
        return None;
    }
    let body = r.text(0)?;
    let issued = r.channel(1)?.as_f64()? as i64;
    let age = r.num(2)?;
    Some((body, issued, age))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    fn ts() -> DataRecord {
        DataRecord {
            schema_id: TIMESERIES_ID.into(),
            schema_version: 1,
            channels: vec![
                DataValue::Int64(1_700_000_000),
                DataValue::Double(282.0),
                DataValue::Double(12.5),
            ],
            t_wall_ns: 0,
            t_sample: None,
            seq: 0,
            record_flags: DataFlags::Ok,
        }
    }

    #[test]
    fn numeric_carriers_widen_and_the_rest_refuse() {
        assert_eq!(DataValue::Single(1.5).as_f64(), Some(1.5));
        assert_eq!(DataValue::Int32(-4).as_f64(), Some(-4.0));
        assert_eq!(DataValue::Text("x".into()).as_f64(), None, "a string is never a silent 0.0");
        assert_eq!(DataValue::Enumerated(3).as_f64(), None);
        assert_eq!(DataValue::Enumerated(3).as_enum(), Some(3));
    }

    #[test]
    fn timeseries_channel_order_is_the_contract() {
        let r = ts();
        assert_eq!(ts_channels(&r), Some((1_700_000_000, 282.0, 12.5)));
        assert!(r.is_observatory());
    }

    #[test]
    fn a_wrong_schema_refuses_rather_than_misreading() {
        let mut r = ts();
        r.schema_id = EVENTS_ID.into(); // same channels, wrong schema
        assert!(ts_channels(&r).is_none(), "reading a timeseries as events must refuse");
        assert!(event_channels(&r).is_none(), "the channels do not fit the events shape");
    }

    #[test]
    fn a_short_record_refuses() {
        let mut r = ts();
        r.channels.pop();
        assert!(ts_channels(&r).is_none(), "a record shorter than its schema is not guessed at");
    }

    #[test]
    fn grid_values_borrow_the_vec_carrier() {
        let r = DataRecord {
            schema_id: GRID_ID.into(),
            schema_version: 1,
            channels: vec![
                DataValue::Int32(2),
                DataValue::Int32(2),
                DataValue::Double(90.0),
                DataValue::Double(-180.0),
                DataValue::Double(2.0),
                DataValue::Vec(vec![1.0, 2.0, f64::NAN, 4.0]),
                DataValue::Int64(500),
                DataValue::Double(1.0),
            ],
            t_wall_ns: 0,
            t_sample: None,
            seq: 0,
            record_flags: DataFlags::Ok,
        };
        let g = grid_channels(&r).unwrap();
        assert_eq!((g.cols, g.rows), (2, 2));
        assert_eq!(g.values.len(), 4);
        assert!(g.values[2].is_nan(), "a missing cell is NaN, never a zero that reads as data");
    }

    #[test]
    fn severity_and_visibility_vocabularies_match_the_broker() {
        // The core's mirrors of the two enum scales — drift-tested against sparq-streams by the
        // harness. The order is the contract (an index means the same tier on both sides).
        assert_eq!(SEVERITY_OPTIONS.len(), 5);
        assert_eq!(SEVERITY_OPTIONS[SEVERITY_SEVERE as usize], "severe");
        assert_eq!(VISIBILITY_OPTIONS.len(), 4);
        assert_eq!(VISIBILITY_OPTIONS[VISIBILITY_NIGHT as usize], "night");
    }
}
