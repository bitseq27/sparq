//! Boundary mapping: the broker's `sparq_streams` types → the core's pure types.
//!
//! This is the harness's half of the D13 split (the wasm glue is the other half): it turns a
//! replayed [`sparq_streams::window::Window`] into the [`CellInput`] the core's [`Wall::draw`]
//! consumes. The mapping is field-for-field (the two `DataRecord`/`DataValue` mirrors are the same
//! frozen vocabulary), and the drift test in `tests/golden.rs` pins the core's streams table and
//! schema constants against the broker's, so the two mirrors cannot silently diverge.

use observatory_core::record::{DataFlags, DataRecord, DataValue};
use observatory_core::view::CellStatus;
use observatory_core::wall::CellInput;
use sparq_streams::record::{DataRecord as SRecord, DataValue as SValue};
use sparq_streams::registry::StreamDef;
use sparq_streams::window::{StreamStatus, Window};

/// Maps a broker [`SValue`] to a core [`DataValue`] (identical carriers, 1:1).
#[must_use]
pub fn to_core_value(v: &SValue) -> DataValue {
    match v {
        SValue::Single(x) => DataValue::Single(*x),
        SValue::Double(x) => DataValue::Double(*x),
        SValue::Int32(n) => DataValue::Int32(*n),
        SValue::Int64(n) => DataValue::Int64(*n),
        SValue::Boolean(b) => DataValue::Boolean(*b),
        SValue::Vec(xs) => DataValue::Vec(xs.clone()),
        SValue::Enumerated(n) => DataValue::Enumerated(*n),
        SValue::Text(s) => DataValue::Text(s.clone()),
        SValue::Blob(b) => DataValue::Blob(b.clone()),
    }
}

/// Maps a broker [`SRecord`] to a core [`DataRecord`].
#[must_use]
pub fn to_core_record(r: &SRecord) -> DataRecord {
    DataRecord {
        schema_id: r.schema_id.clone(),
        schema_version: r.schema_version,
        channels: r.channels.iter().map(to_core_value).collect(),
        t_wall_ns: r.t_wall_ns,
        t_sample: r.t_sample,
        seq: r.seq,
        record_flags: to_core_flags(r.record_flags),
    }
}

/// Maps the broker's record flags to the core's.
#[must_use]
pub fn to_core_flags(f: sparq_streams::record::DataFlags) -> DataFlags {
    match f {
        sparq_streams::record::DataFlags::Ok => DataFlags::Ok,
        sparq_streams::record::DataFlags::Stale => DataFlags::Stale,
        sparq_streams::record::DataFlags::Discontinuity => DataFlags::Discontinuity,
        sparq_streams::record::DataFlags::Estimated => DataFlags::Estimated,
    }
}

/// The core records of a window, oldest → newest (the [`Window::iter`] order).
#[must_use]
pub fn window_records(w: &Window) -> Vec<DataRecord> {
    w.iter().map(to_core_record).collect()
}

/// Maps the broker's freshness to the core's cell status, honouring D14: a key-requiring stream with
/// no delivered data reads KEY NEEDED (naming the env fix), never a silent OFFLINE.
#[must_use]
pub fn cell_status(w: &Window, def: Option<&StreamDef>, now_unix: i64) -> CellStatus {
    let empty = w.is_empty();
    if empty {
        if let Some(d) = def {
            if d.requires_key() {
                // The env var name to set — the registry carries it (D14's "in words" fix).
                return CellStatus::KeyNeeded(leak_env(d.key_env.as_deref()));
            }
        }
        return CellStatus::Offline;
    }
    match w.status(now_unix) {
        StreamStatus::Live => CellStatus::Live,
        StreamStatus::Stale => CellStatus::Stale,
        StreamStatus::Offline => CellStatus::Offline,
    }
}

/// The core's `CellStatus::KeyNeeded` carries a `&'static str` (the guest has no allocator for env
/// names at draw); the harness leaks the registry's env name (a handful of bytes, once per render —
/// a test/bench harness, not the shipped guest, so the leak is bounded and honest).
fn leak_env(env: Option<&str>) -> &'static str {
    Box::leak(env.unwrap_or("SPARQ_API_KEY").to_string().into_boxed_str())
}

/// Builds the per-cell (status, records) pairs for a wall from the replayed windows and the params'
/// cell→stream resolution. Returns OWNED data; the caller assembles the borrowed [`CellInput`]s in
/// one scope (so the record storage outlives the inputs). `windows` maps stream id → replayed window.
#[must_use]
pub fn build_cell_data(
    params: &observatory_core::params::Params,
    windows: &std::collections::HashMap<String, Window>,
    registry: &sparq_streams::registry::Registry,
    now_unix: i64,
) -> Vec<(CellStatus, Vec<DataRecord>)> {
    (0..observatory_core::CELL_COUNT)
        .map(|i| match params.cell_stream(i).and_then(|id| windows.get(id).map(|w| (id, w))) {
            Some((id, w)) => {
                let def = registry.get(id);
                (cell_status(w, def, now_unix), window_records(w))
            },
            None => (CellStatus::Offline, Vec::new()),
        })
        .collect()
}

/// Assembles the borrowed [`CellInput`] slice from owned per-cell data (a one-liner the caller uses
/// inside the scope where `data` lives, so the borrows are valid for the `draw` call).
#[must_use]
pub fn as_inputs(data: &[(CellStatus, Vec<DataRecord>)]) -> Vec<CellInput<'_>> {
    data.iter().map(|(status, recs)| CellInput { status: *status, records: recs }).collect()
}
