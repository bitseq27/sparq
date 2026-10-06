//! Rolling windows and the stale/offline policy — the broker's short-term memory.
//!
//! Each stream keeps a bounded window of its most recent [`DataRecord`]s, sized by stream kind
//! (plan D10): a timeseries keeps 2048 samples (a phosphor trace's worth), an event feed keeps 256,
//! a heat grid keeps just the 2 newest frames (grids are big and only the latest is drawn), a text
//! feed keeps 64 bulletins. Eviction is oldest-first, so [`Window::iter`] reads oldest → newest — the
//! same "newest last" convention the frozen `ring-snapshot` uses (sources.wit), which is what lets a
//! renderer phase-align a trace without reversing it.
//!
//! # The clock is injected, never read
//!
//! Nothing here calls `SystemTime::now` (forbidden workspace-wide by `clippy.toml` on the audio
//! path, and forbidden *here* by the determinism firewall, plan D11). Freshness is a pure function
//! of an injected `now_unix` and the window's last-fetch stamp, so the stale/offline transitions are
//! bit-exact testable and the same window serves a golden today and tomorrow. Only the live
//! transport half (INC5) reads a clock, at fetch time, host-side.
//!
//! # Stale policy (plan §6.1, D10)
//!
//! With `age = now - last_fetch` and `c = cadence_s`:
//! * never fetched, or `age ≥ 10c` → [`StreamStatus::Offline`]
//! * `3c ≤ age < 10c` → [`StreamStatus::Stale`]
//! * `age < 3c` → [`StreamStatus::Live`]
//!
//! The status surfaces twice, always together: as the served records' [`DataFlags::Stale`] *and* as
//! the LED word (§5.3) — a flag without its word, or a word without its flag, is the silent-hole bug
//! the plan refuses.

use std::collections::VecDeque;

use crate::record::{DataFlags, DataRecord};
use crate::registry::StreamDef;
use crate::schema;

/// The four window kinds and their record capacities (plan D10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowKind {
    /// A scalar or position trail: 2048 records (a phosphor trace's worth of history).
    Timeseries,
    /// A discrete-event feed: 256 records.
    Events,
    /// A heat grid: the 2 newest frames only (grids are large; the wall draws the latest).
    Grid,
    /// A raw-text feed: 64 bulletins.
    Text,
}

impl WindowKind {
    /// The record capacity for this kind (plan D10).
    #[must_use]
    pub fn capacity(self) -> usize {
        match self {
            Self::Timeseries => 2048,
            Self::Events => 256,
            Self::Grid => 2,
            Self::Text => 64,
        }
    }

    /// Maps a schema id to its window kind. The position schema windows as a [`Self::Timeseries`]
    /// trail on purpose: the ISS feeds both a live map marker (the newest record) and an alt/vel
    /// trace (§5.5), so it keeps a trail, not a single frame. The five schemas collapse to the four
    /// window kinds the plan declares.
    #[must_use]
    pub fn from_schema(schema_id: &str) -> Option<Self> {
        match schema_id {
            schema::TIMESERIES_ID | schema::POSITION_ID => Some(Self::Timeseries),
            schema::EVENTS_ID => Some(Self::Events),
            schema::GRID_ID => Some(Self::Grid),
            schema::TEXT_ID => Some(Self::Text),
            _ => None,
        }
    }
}

/// The freshness of a stream, from the stale policy above. KEY NEEDED is a *separate* axis (the
/// registry's [`crate::registry::KeyState`]), not a freshness: a cell LED reads KEY NEEDED when the
/// stream is unfetchable for want of a key, else one of these three words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamStatus {
    /// A fresh record within 3 × cadence.
    Live,
    /// No fresh record for 3–10 × cadence: served records carry [`DataFlags::Stale`].
    Stale,
    /// No fresh record for ≥ 10 × cadence, or never fetched, or manually disabled.
    Offline,
}

impl StreamStatus {
    /// The LED word (§5.3). Always a word, never a bare colour.
    #[must_use]
    pub fn as_words(self) -> &'static str {
        match self {
            Self::Live => "LIVE",
            Self::Stale => "STALE",
            Self::Offline => "OFFLINE",
        }
    }

    /// The record flag this status stamps onto served records (§6.1: the flag and the word are the
    /// same statement in two vocabularies).
    #[must_use]
    pub fn record_flag(self) -> DataFlags {
        match self {
            Self::Live => DataFlags::Ok,
            Self::Stale | Self::Offline => DataFlags::Stale,
        }
    }
}

/// A bounded, clock-free rolling window of one stream's records plus its freshness stamp.
#[derive(Clone, Debug)]
pub struct Window {
    kind: WindowKind,
    cadence_s: u32,
    records: VecDeque<DataRecord>,
    last_fetch_unix: Option<i64>,
}

impl Window {
    /// An empty window of `kind`, with the given poll cadence (which sets the stale horizon).
    #[must_use]
    pub fn new(kind: WindowKind, cadence_s: u32) -> Self {
        Self { kind, cadence_s: cadence_s.max(1), records: VecDeque::new(), last_fetch_unix: None }
    }

    /// Builds the right window for a registry stream: kind from its schema, cadence from its row. An
    /// unrecognised schema (impossible for a loaded registry, which validates schemas) falls back to
    /// the smallest kind rather than panicking — a library that panics on its own data cannot be
    /// trusted at a boundary.
    #[must_use]
    pub fn for_stream(def: &StreamDef) -> Self {
        let kind = def
            .schema_parts()
            .and_then(|(id, _)| WindowKind::from_schema(id))
            .unwrap_or(WindowKind::Text);
        Self::new(kind, def.cadence_s)
    }

    /// The window kind.
    #[must_use]
    pub fn kind(&self) -> WindowKind {
        self.kind
    }

    /// The record capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.kind.capacity()
    }

    /// Appends one record, evicting oldest-first past capacity.
    pub fn push(&mut self, record: DataRecord) {
        self.records.push_back(record);
        while self.records.len() > self.kind.capacity() {
            self.records.pop_front();
        }
    }

    /// Appends a batch of records (a normalizer's whole payload at once), evicting oldest-first.
    pub fn extend<I: IntoIterator<Item = DataRecord>>(&mut self, records: I) {
        for r in records {
            self.push(r);
        }
    }

    /// Stamps the last successful fetch at `now_unix`. This — not the records' own times — drives
    /// the stale policy: a feed can serve old records but still be LIVE if the broker just reached
    /// it, and a feed whose records look recent is OFFLINE if the broker has not reached it in 10 ×
    /// cadence.
    pub fn mark_fetched(&mut self, now_unix: i64) {
        self.last_fetch_unix = Some(now_unix);
    }

    /// The unix time of the last successful fetch, if any.
    #[must_use]
    pub fn last_fetch_unix(&self) -> Option<i64> {
        self.last_fetch_unix
    }

    /// An iterator over the records, oldest → newest.
    pub fn iter(&self) -> impl Iterator<Item = &DataRecord> {
        self.records.iter()
    }

    /// The newest record, if any.
    #[must_use]
    pub fn latest(&self) -> Option<&DataRecord> {
        self.records.back()
    }

    /// The number of records held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the window is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The freshness at `now_unix`, per the stale policy in the module docs.
    #[must_use]
    pub fn status(&self, now_unix: i64) -> StreamStatus {
        let cadence = i64::from(self.cadence_s);
        match self.last_fetch_unix {
            None => StreamStatus::Offline,
            Some(last) => {
                let age = now_unix.saturating_sub(last);
                if age >= 10 * cadence {
                    StreamStatus::Offline
                } else if age >= 3 * cadence {
                    StreamStatus::Stale
                } else {
                    StreamStatus::Live
                }
            },
        }
    }

    /// The records as the host would serve them at `now_unix`: a contiguous oldest → newest `Vec`
    /// whose flags carry the freshness ([`DataFlags::Stale`] when not LIVE), so the flag and the LED
    /// word never disagree. This is the window's read face; it allocates, which is legal off the
    /// audio thread (the broker and the UI thread are control threads, plan D3/D5).
    #[must_use]
    pub fn snapshot(&self, now_unix: i64) -> Vec<DataRecord> {
        let flag = self.status(now_unix).record_flag();
        self.records
            .iter()
            .map(|r| if flag == DataFlags::Ok { r.clone() } else { r.clone().with_flags(flag) })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::record::DataValue;
    use crate::schema;

    fn rec(t: i64) -> DataRecord {
        schema::timeseries(t, t as f64, 0.0)
    }

    #[test]
    fn window_kinds_carry_the_plan_capacities() {
        assert_eq!(WindowKind::Timeseries.capacity(), 2048);
        assert_eq!(WindowKind::Events.capacity(), 256);
        assert_eq!(WindowKind::Grid.capacity(), 2);
        assert_eq!(WindowKind::Text.capacity(), 64);
    }

    #[test]
    fn the_five_schemas_map_onto_the_four_window_kinds() {
        assert_eq!(WindowKind::from_schema(schema::TIMESERIES_ID), Some(WindowKind::Timeseries));
        assert_eq!(WindowKind::from_schema(schema::POSITION_ID), Some(WindowKind::Timeseries));
        assert_eq!(WindowKind::from_schema(schema::EVENTS_ID), Some(WindowKind::Events));
        assert_eq!(WindowKind::from_schema(schema::GRID_ID), Some(WindowKind::Grid));
        assert_eq!(WindowKind::from_schema(schema::TEXT_ID), Some(WindowKind::Text));
        assert_eq!(WindowKind::from_schema("sparq/imu9"), None);
    }

    #[test]
    fn eviction_is_oldest_first_and_bounded() {
        let mut w = Window::new(WindowKind::Grid, 300); // capacity 2
        w.push(rec(1));
        w.push(rec(2));
        w.push(rec(3));
        assert_eq!(w.len(), 2);
        assert_eq!(w.latest().and_then(|r| r.channel(0)), Some(&DataValue::Int64(3)));
        let times: Vec<i64> = w
            .iter()
            .filter_map(|r| match r.channel(0) {
                Some(DataValue::Int64(t)) => Some(*t),
                _ => None,
            })
            .collect();
        assert_eq!(times, vec![2, 3], "oldest evicted, order preserved oldest→newest");
    }

    #[test]
    fn a_big_window_keeps_the_newest_2048() {
        let mut w = Window::new(WindowKind::Timeseries, 60);
        for t in 0..3000 {
            w.push(rec(t));
        }
        assert_eq!(w.len(), 2048);
        assert_eq!(w.latest().and_then(|r| r.channel(0)), Some(&DataValue::Int64(2999)));
    }

    #[test]
    fn never_fetched_is_offline() {
        let w = Window::new(WindowKind::Timeseries, 60);
        assert_eq!(w.status(1_000_000), StreamStatus::Offline);
        assert_eq!(w.status(1_000_000).as_words(), "OFFLINE");
    }

    #[test]
    fn stale_transitions_at_3x_and_offline_at_10x_cadence() {
        let mut w = Window::new(WindowKind::Timeseries, 60);
        w.mark_fetched(1000);
        // age < 180 → LIVE
        assert_eq!(w.status(1000), StreamStatus::Live);
        assert_eq!(w.status(1179), StreamStatus::Live);
        // 180 ≤ age < 600 → STALE
        assert_eq!(w.status(1180), StreamStatus::Stale);
        assert_eq!(w.status(1599), StreamStatus::Stale);
        // age ≥ 600 → OFFLINE
        assert_eq!(w.status(1600), StreamStatus::Offline);
        assert_eq!(w.status(9999), StreamStatus::Offline);
    }

    #[test]
    fn snapshot_flags_records_stale_when_not_live() {
        let mut w = Window::new(WindowKind::Timeseries, 60);
        w.push(rec(1000));
        w.mark_fetched(1000);
        // LIVE: flags stay Ok.
        assert_eq!(w.snapshot(1010)[0].record_flags, DataFlags::Ok);
        // STALE: served records carry the Stale flag (and the LED would read STALE).
        let stale = w.snapshot(1200);
        assert_eq!(stale[0].record_flags, DataFlags::Stale);
        assert_eq!(w.status(1200).as_words(), "STALE");
        assert_eq!(w.status(1200).record_flag(), DataFlags::Stale);
    }

    #[test]
    fn for_stream_picks_kind_and_cadence_from_the_registry() {
        let reg = crate::registry::Registry::load().unwrap();
        let iss = reg.get("sat.iss").unwrap();
        let w = Window::for_stream(iss);
        assert_eq!(w.kind(), WindowKind::Timeseries, "position windows as a trail");
        let aurora = reg.get("swpc.aurora").unwrap();
        assert_eq!(Window::for_stream(aurora).kind(), WindowKind::Grid);
        assert_eq!(Window::for_stream(aurora).capacity(), 2);
    }

    #[test]
    fn a_zero_cadence_is_clamped_so_the_policy_never_divides_by_zero() {
        let mut w = Window::new(WindowKind::Timeseries, 0);
        w.mark_fetched(1000);
        // cadence clamps to 1, so 3*1=3 and 10*1=10 are the horizons.
        assert_eq!(w.status(1002), StreamStatus::Live);
        assert_eq!(w.status(1003), StreamStatus::Stale);
        assert_eq!(w.status(1010), StreamStatus::Offline);
    }
}
