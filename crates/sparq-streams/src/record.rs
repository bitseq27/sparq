//! The frozen `data-record` vocabulary, as native Rust.
//!
//! This is the third representation of a contract whose truth lives in
//! `docs/api/instrument-wit/wit/types.wit` (the guest face) and the schema doc (the prose). It is
//! a **field-for-field mirror** of the WIT `data-value` / `data-flags` / `data-record` types — the
//! v1.1 freeze set — because the broker produces exactly these records and the host serves them
//! through the contract's own door (`sources.snapshot` → `window(list<data-record>)`, sources.wit).
//!
//! The WIT types.wit marks data records "DOCUMENT-ARBITED, not code-arbited … no native arbiter
//! yet": the native `data`-port routing is Phase 5. This module is *not* that arbiter and does not
//! claim to be — it is the broker's own hermetic mirror, so the stream plane can be built, tested
//! and proven against the frozen vocabulary before the routing lands. When Phase 5's native
//! data-port types arrive, this mirror aligns to them (the plan's review trigger, ADR-011 §e); it
//! does not compete with them.
//!
//! # Stamping (ADR-007, sources.wit decision C)
//!
//! On the wire every stamp is host-owned: the broker's `t_wall_ns` and `seq` are **advisory** and
//! the host re-stamps both when it serves a window, because a guest- or broker-readable clock that
//! reached the journal would make replay diverge. The observatory carries `age_s` as an ordinary
//! `double` channel *inside* the record (the plan's D8: the host may read its own clock, the guest
//! may not) so the wall can show an age without a wall clock of its own.

/// One typed channel value — the frozen `data-value` variant, 1:1 with `types.wit`.
///
/// The carriers are closed: a stream that cannot be expressed in these nine is not normalized into
/// a record at all (it is refused in words at the registry/normalizer boundary), never squeezed
/// into the wrong carrier. `Vec` uses the f64 carrier — the one judgement call the v1.1 freeze
/// ratified (operator ruling 2026-10-05), recorded here so the next instrument does not re-litigate
/// it.
#[derive(Clone, Debug, PartialEq)]
pub enum DataValue {
    /// dtype `f32` — a single-precision measurement.
    Single(f32),
    /// dtype `f64` — a double-precision measurement (the observatory's default carrier).
    Double(f64),
    /// dtype `i32` — a count or small integer.
    Int32(i32),
    /// dtype `i64` — a wide integer (unix seconds ride here).
    Int64(i64),
    /// dtype `bool`.
    Boolean(bool),
    /// dtype `vecN` — a fixed-N vector on the f64 carrier (grid rows, spectra); N is declared by
    /// the schema, the carrier width was ratified f64 at the v1.1 freeze.
    Vec(Vec<f64>),
    /// dtype `enum` — an index into the schema's declared options (never a raw value).
    Enumerated(u32),
    /// dtype `str` — a headline, a raw feed line, a place name.
    Text(String),
    /// dtype `blob` — opaque bytes (TLE blocks ride here when SGP4 lands, INC6).
    Blob(Vec<u8>),
}

impl DataValue {
    /// The dtype name, as the schema doc spells it — for diagnostics and refusal words.
    #[must_use]
    pub fn dtype(&self) -> &'static str {
        match self {
            Self::Single(_) => "f32",
            Self::Double(_) => "f64",
            Self::Int32(_) => "i32",
            Self::Int64(_) => "i64",
            Self::Boolean(_) => "bool",
            Self::Vec(_) => "vecN",
            Self::Enumerated(_) => "enum",
            Self::Text(_) => "str",
            Self::Blob(_) => "blob",
        }
    }

    /// The value as an `f64`, for the numeric carriers only (`Single` widens, `Double`/`Int32`/
    /// `Int64` widen). Text/enum/vec/blob/bool answer `None` — a silent 0.0 for a non-number is
    /// exactly the wrong-sound-with-no-symptom bug the contract forbids.
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
}

/// Record quality flags — the frozen `data-flags` enum, 1:1 with `types.wit`.
///
/// The WIT field is spelled `record-flags` because `flags` is a WIT keyword; the native name keeps
/// the same meaning. `Stale` is the one the observatory surfaces most: a stream that has not
/// produced a fresh record within 3 × its cadence is served with `Stale` records *and* the word
/// STALE in its cell LED (plan §5.3, D10) — the flag and the words are the same statement in two
/// vocabularies, never one without the other.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DataFlags {
    /// Fresh and complete, as fetched.
    #[default]
    Ok,
    /// Older than the stream's stale horizon (3 × cadence without a fresh record).
    Stale,
    /// A gap in the sequence — the broker missed or dropped a fetch between neighbours.
    Discontinuity,
    /// Interpolated or modelled, not directly observed (e.g. a forecast cell).
    Estimated,
}

impl DataFlags {
    /// The LED word the shell draws for this flag (plan §5.3). Always a word, never a bare colour:
    /// the refuse-in-words rule reaches the data flags too.
    #[must_use]
    pub fn as_words(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Stale => "STALE",
            Self::Discontinuity => "GAP",
            Self::Estimated => "ESTIMATED",
        }
    }
}

/// One typed data record — the frozen `data-record`, 1:1 with `types.wit`.
///
/// Channel order follows the record's `schema_id`@`schema_version` (see [`crate::schema`]); a
/// consumer that does not know the schema must not guess the channel meanings — unknown schemas are
/// a load-time error, never a runtime surprise (the contract's own words).
#[derive(Clone, Debug, PartialEq)]
pub struct DataRecord {
    /// Schema identity, e.g. `observatory/timeseries` (the `@version` rides `schema_version`).
    pub schema_id: String,
    /// Schema version, e.g. `1`.
    pub schema_version: u32,
    /// The channel values, in the schema's declared order.
    pub channels: Vec<DataValue>,
    /// Wall-clock stamp in nanoseconds. **Advisory from the broker**: the host re-stamps it when it
    /// serves the window (sources.wit decision C). The broker fills it from the record's own UTC
    /// time so the value is meaningful before the host re-stamps.
    pub t_wall_ns: u64,
    /// Audio-time sample, present only when the schema's clock domain is audio. The observatory's
    /// schemas are all wall-clock, so this is `None` on every record the broker produces.
    pub t_sample: Option<u64>,
    /// Sequence number. **Advisory from the broker**; the host re-assigns it at the block boundary.
    pub seq: u64,
    /// Quality/freshness flags for this record.
    pub record_flags: DataFlags,
}

impl DataRecord {
    /// Builds a record from a schema identity and its channels, with advisory stamps zeroed and
    /// flags `Ok`. The host owns the authoritative `t_wall_ns`/`seq`; the broker leaves them for the
    /// host to fill and sets `t_wall_ns` from the record's own time via [`DataRecord::stamped`].
    #[must_use]
    pub fn new(schema_id: &str, schema_version: u32, channels: Vec<DataValue>) -> Self {
        Self {
            schema_id: schema_id.to_string(),
            schema_version,
            channels,
            t_wall_ns: 0,
            t_sample: None,
            seq: 0,
            record_flags: DataFlags::Ok,
        }
    }

    /// The same record with its advisory wall-clock stamp set from a unix-seconds time. The broker
    /// calls this so a record is self-describing before the host re-stamps it; the host's stamp
    /// still wins on the wire.
    #[must_use]
    pub fn stamped(mut self, t_utc_s: i64) -> Self {
        // Unix seconds -> nanoseconds. Negative (pre-1970) times clamp to 0 rather than wrap: a
        // feed that reports a pre-epoch stamp is malformed, and 0 reads as "very stale", which is
        // the honest interpretation, not a wrapped future date.
        self.t_wall_ns =
            if t_utc_s > 0 { (t_utc_s as u64).saturating_mul(1_000_000_000) } else { 0 };
        self
    }

    /// Returns a copy carrying `flags`, leaving the rest untouched. Used by the window when it
    /// serves a stream that has gone stale (the flag and the LED word are the same statement).
    #[must_use]
    pub fn with_flags(mut self, flags: DataFlags) -> Self {
        self.record_flags = flags;
        self
    }

    /// The channel at `i`, or `None` if the record is shorter than the schema expects. A short
    /// record is a normalizer bug the consumer must not paper over with a default.
    #[must_use]
    pub fn channel(&self, i: usize) -> Option<&DataValue> {
        self.channels.get(i)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn dtype_names_match_the_schema_doc() {
        assert_eq!(DataValue::Single(0.0).dtype(), "f32");
        assert_eq!(DataValue::Double(0.0).dtype(), "f64");
        assert_eq!(DataValue::Int32(0).dtype(), "i32");
        assert_eq!(DataValue::Int64(0).dtype(), "i64");
        assert_eq!(DataValue::Boolean(true).dtype(), "bool");
        assert_eq!(DataValue::Vec(vec![]).dtype(), "vecN");
        assert_eq!(DataValue::Enumerated(0).dtype(), "enum");
        assert_eq!(DataValue::Text(String::new()).dtype(), "str");
        assert_eq!(DataValue::Blob(vec![]).dtype(), "blob");
    }

    #[test]
    fn as_f64_widens_numbers_and_refuses_the_rest() {
        assert_eq!(DataValue::Single(1.5).as_f64(), Some(1.5));
        assert_eq!(DataValue::Int32(-4).as_f64(), Some(-4.0));
        assert_eq!(DataValue::Int64(7).as_f64(), Some(7.0));
        assert_eq!(DataValue::Text("nope".into()).as_f64(), None);
        assert_eq!(DataValue::Enumerated(3).as_f64(), None);
    }

    #[test]
    fn stamping_converts_unix_seconds_and_clamps_pre_epoch() {
        let r = DataRecord::new("observatory/text", 1, vec![]).stamped(1_700_000_000);
        assert_eq!(r.t_wall_ns, 1_700_000_000_000_000_000);
        let bad = DataRecord::new("observatory/text", 1, vec![]).stamped(-5);
        assert_eq!(
            bad.t_wall_ns, 0,
            "a pre-epoch stamp reads as very stale, never a wrapped future"
        );
    }

    #[test]
    fn flags_carry_words_not_just_colours() {
        assert_eq!(DataFlags::Ok.as_words(), "OK");
        assert_eq!(DataFlags::Stale.as_words(), "STALE");
        assert_eq!(DataFlags::Discontinuity.as_words(), "GAP");
        assert_eq!(DataFlags::Estimated.as_words(), "ESTIMATED");
        assert_eq!(DataFlags::default(), DataFlags::Ok);
    }

    #[test]
    fn with_flags_leaves_everything_else_alone() {
        let r = DataRecord::new("observatory/grid", 1, vec![DataValue::Int32(2)]).stamped(100);
        let s = r.clone().with_flags(DataFlags::Stale);
        assert_eq!(s.record_flags, DataFlags::Stale);
        assert_eq!(s.t_wall_ns, r.t_wall_ns);
        assert_eq!(s.channels, r.channels);
    }
}
