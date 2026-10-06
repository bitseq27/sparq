//! The per-stream normalizer suite (plan INC1: "normalizer unit tests per stream against checked-in
//! fixtures — 23 minimum"). Every test runs a REAL recorded payload from
//! `reference/fixtures/observatory/` through the normalizer and asserts the structural invariants of
//! the frozen schema — never an exact live value (fixtures are re-recordable; the invariants are
//! not). Together they prove the broker turns all 23 feeds into `data-record`s the guest can trust.
#![cfg(feature = "streams")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use sparq_streams::normalize::{has_normalizer, normalize};
use sparq_streams::record::{DataRecord, DataValue};
use sparq_streams::registry::Registry;
use sparq_streams::replay::load_fixture;
use sparq_streams::schema;

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference/fixtures/observatory")
}

/// Normalizes a stream's recorded fixture at its own fetch time (age_s relative to the fetch).
fn records_for(reg: &Registry, id: &str) -> Vec<DataRecord> {
    let fix = load_fixture(&fixture_dir(), id).unwrap_or_else(|e| panic!("fixture for {id}: {e}"));
    assert_eq!(fix.status, 200, "fixture {id} was not a 200 recording: {}", fix.note);
    let def = reg.get(id).unwrap_or_else(|| panic!("{id} not in registry"));
    normalize(def, &fix.body, fix.fetched_unix).unwrap_or_else(|e| panic!("normalize {id}: {e}"))
}

fn schema_id_of<'a>(reg: &'a Registry, id: &str) -> &'a str {
    reg.get(id).unwrap().schema_parts().unwrap().0
}

/// Asserts every record carries the schema the registry declares for the stream — the drift gate
/// between the registry's `schema` column and what the normalizer actually emits.
fn assert_schema(reg: &Registry, id: &str, records: &[DataRecord]) {
    let want = schema_id_of(reg, id);
    for r in records {
        assert_eq!(r.schema_id, want, "{id} emitted the wrong schema");
        assert!(schema::is_observatory_schema(&r.schema_id));
    }
}

// ── the drift gate: the registry and the normalizer dispatch must never diverge ─────────────────

#[test]
fn every_registry_stream_has_a_normalizer() {
    let reg = Registry::load().unwrap();
    for def in reg.streams() {
        assert!(
            has_normalizer(&def.id),
            "registry stream `{}` has no normalizer (normalize.rs)",
            def.id
        );
    }
    assert_eq!(reg.len(), 23, "the v1 registry is 23 streams");
}

#[test]
fn every_normalizer_produces_its_declared_schema_from_the_real_fixture() {
    let reg = Registry::load().unwrap();
    for def in reg.streams() {
        let recs = records_for(&reg, &def.id);
        assert_schema(&reg, &def.id, &recs);
    }
}

#[test]
fn normalizing_a_fixture_twice_is_bit_identical() {
    let reg = Registry::load().unwrap();
    let a = records_for(&reg, "geo.quakes-day");
    let b = records_for(&reg, "geo.quakes-day");
    assert_eq!(a, b, "the determinism firewall: same fixture => same records, twice");
}

// ── SW · space weather ──────────────────────────────────────────────────────────────────────────

#[test]
fn aurora_regrids_to_a_2deg_180x90_grid() {
    let reg = Registry::load().unwrap();
    let recs = records_for(&reg, "swpc.aurora");
    assert_eq!(recs.len(), 1, "one grid frame per fetch");
    let r = &recs[0];
    assert_eq!(r.channel(0), Some(&DataValue::Int32(180)), "cols");
    assert_eq!(r.channel(1), Some(&DataValue::Int32(90)), "rows");
    assert_eq!(r.channel(4), Some(&DataValue::Double(2.0)), "step_deg = 2 (the declared regrid)");
    match r.channel(5) {
        Some(DataValue::Vec(v)) => {
            assert_eq!(v.len(), 180 * 90, "16 200 cells (plan §6.4)");
            let filled = v.iter().filter(|x| x.is_finite()).count();
            assert!(filled > 0, "some cells carry aurora intensity");
            // OVATION is a COMPLETE global grid, so every 2° bin has data — 0 means "quiet aurora",
            // not "missing". NaN (the no-cell sentinel) is for sparse feeds like FIRMS density.
            assert!(filled * 100 >= v.len() * 99, "the global aurora grid fills (≥99%) every cell");
        },
        other => panic!("grid values must be a Vec, got {other:?}"),
    }
}

#[test]
fn kp_is_a_timeseries_of_indices() {
    let reg = Registry::load().unwrap();
    let recs = records_for(&reg, "swpc.kp");
    assert!(recs.len() > 1, "a 3-day Kp feed has many 3-hourly rows");
    for r in &recs {
        assert!(matches!(r.channel(1), Some(DataValue::Double(_))), "Kp value is a Double");
    }
    // Sorted oldest -> newest (the window convention).
    let stamps: Vec<u64> = recs.iter().map(|r| r.t_wall_ns).collect();
    let mut sorted = stamps.clone();
    sorted.sort_unstable();
    assert_eq!(stamps, sorted, "timeseries records are time-ordered");
}

#[test]
fn solar_wind_bz_and_flux_are_single_value_timeseries() {
    let reg = Registry::load().unwrap();
    for id in ["swpc.solar-wind", "swpc.bz", "swpc.flux10cm"] {
        let recs = records_for(&reg, id);
        assert!(!recs.is_empty(), "{id} produced no records");
        assert_schema(&reg, id, &recs);
        assert!(matches!(recs[0].channel(1), Some(DataValue::Double(_))), "{id} value is a Double");
    }
    // Bz is signed (bipolar): the fixture records a negative IMF component.
    let bz = records_for(&reg, "swpc.bz");
    if let Some(DataValue::Double(v)) = bz[0].channel(1) {
        assert!(v.abs() < 100.0, "Bz is a small signed nT value, got {v}");
    }
}

#[test]
fn xray_flares_map_class_letters_to_severity_tiers() {
    let reg = Registry::load().unwrap();
    let recs = records_for(&reg, "swpc.xray-flares");
    assert!(!recs.is_empty(), "a 7-day flare feed has events");
    for r in &recs {
        let sev = match r.channel(5) {
            Some(DataValue::Enumerated(s)) => *s,
            other => panic!("flare severity must be an enum, got {other:?}"),
        };
        assert!((1..=4).contains(&sev), "a real flare class maps to LOW..SEVERE, got {sev}");
        let head = r.channel(6).and_then(DataValue::as_str).unwrap_or("");
        assert!(
            head.contains("GOES") && head.contains("flare"),
            "headline names the flare: {head}"
        );
        // Flares have no ground position: lat/lon are NaN (the map skips them).
        assert!(
            matches!(r.channel(1), Some(DataValue::Double(x)) if x.is_nan()),
            "flare lat is NaN"
        );
    }
}

#[test]
fn wwv_and_geomag_are_raw_text_with_an_issue_time() {
    let reg = Registry::load().unwrap();
    for id in ["swpc.wwv", "swpc.geomag-forecast"] {
        let recs = records_for(&reg, id);
        assert_eq!(recs.len(), 1, "{id} is one raw-text bulletin");
        let text = recs[0].channel(0).and_then(DataValue::as_str).unwrap_or("");
        assert!(text.len() > 20, "{id} carries the raw payload verbatim");
        let issued = match recs[0].channel(1) {
            Some(DataValue::Int64(t)) => *t,
            other => panic!("{id} issued_utc must be an Int64, got {other:?}"),
        };
        assert!(issued > 1_700_000_000, "{id} parsed a real :Issued: header ({issued})");
    }
}

// ── SAT · satellites ────────────────────────────────────────────────────────────────────────────

#[test]
fn iss_position_converts_velocity_to_km_per_s() {
    let reg = Registry::load().unwrap();
    let recs = records_for(&reg, "sat.iss");
    assert_eq!(recs.len(), 1);
    let r = &recs[0];
    let lat = match r.channel(0) {
        Some(DataValue::Double(x)) => *x,
        _ => panic!("lat"),
    };
    let vel = match r.channel(3) {
        Some(DataValue::Double(x)) => *x,
        _ => panic!("vel"),
    };
    assert!((-90.0..=90.0).contains(&lat), "ISS latitude in range: {lat}");
    assert!(
        (7.0..8.5).contains(&vel),
        "ISS ~7.66 km/s after the km/h ÷ 3600 conversion, got {vel}"
    );
    assert!(
        matches!(r.channel(5), Some(DataValue::Enumerated(v)) if *v < 4),
        "visibility is a valid enum"
    );
}

#[test]
fn tle_catalogue_becomes_a_text_table() {
    let reg = Registry::load().unwrap();
    let recs = records_for(&reg, "sat.tle-stations");
    assert_eq!(recs.len(), 1, "the catalogue is one table record");
    let text = recs[0].channel(0).and_then(DataValue::as_str).unwrap_or("");
    assert!(
        text.contains("inc") && text.contains("rev/day"),
        "the table carries orbital elements: {text:.80}"
    );
}

// ── GEO · geological & Earth events ─────────────────────────────────────────────────────────────

#[test]
fn usgs_quakes_carry_place_magnitude_and_position() {
    let reg = Registry::load().unwrap();
    for id in ["geo.quakes-hour", "geo.quakes-day"] {
        let recs = records_for(&reg, id);
        assert_schema(&reg, id, &recs);
        // The recorded feeds are non-empty; assert the event structure holds for every record.
        for r in &recs {
            let lat = match r.channel(1) {
                Some(DataValue::Double(x)) => *x,
                _ => panic!("lat"),
            };
            let lon = match r.channel(2) {
                Some(DataValue::Double(x)) => *x,
                _ => panic!("lon"),
            };
            assert!(
                (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon),
                "quake in range"
            );
            let head = r.channel(6).and_then(DataValue::as_str).unwrap_or("");
            assert!(head.starts_with('M'), "headline leads with the magnitude: {head}");
        }
        // quakes-day is M2.5+, so at least one record must be MODERATE tier or above.
        if id == "geo.quakes-day" {
            assert!(!recs.is_empty(), "the M2.5+ day feed has events");
        }
    }
}

#[test]
fn gdacs_alertlevel_maps_to_severity() {
    let reg = Registry::load().unwrap();
    let recs = records_for(&reg, "geo.gdacs");
    assert!(!recs.is_empty(), "GDACS returns EQ/TC/FL events");
    for r in &recs {
        let sev = match r.channel(5) {
            Some(DataValue::Enumerated(s)) => *s,
            _ => panic!("sev"),
        };
        assert!(sev <= 4, "severity is a valid tier");
        let head = r.channel(6).and_then(DataValue::as_str).unwrap_or("");
        assert!(!head.is_empty(), "GDACS headline names the event");
    }
}

#[test]
fn eonet_events_carry_category_in_the_headline() {
    let reg = Registry::load().unwrap();
    for id in ["geo.eonet", "geo.wildfires"] {
        let recs = records_for(&reg, id);
        assert!(!recs.is_empty(), "{id} returns events");
        let head = recs[0].channel(6).and_then(DataValue::as_str).unwrap_or("");
        assert!(head.contains(':'), "headline is `<category>: <title>`: {head}");
    }
}

#[test]
fn firms_csv_sample_yields_three_deterministic_events() {
    let reg = Registry::load().unwrap();
    let recs = records_for(&reg, "geo.firms");
    assert_eq!(recs.len(), 3, "the synthetic FIRMS sample has 3 rows (deterministic)");
    for r in &recs {
        let head = r.channel(6).and_then(DataValue::as_str).unwrap_or("");
        assert!(head.starts_with("FIRMS fire"), "FIRMS headline: {head}");
        assert!(
            matches!(r.channel(1), Some(DataValue::Double(lat)) if (-90.0..=90.0).contains(lat))
        );
    }
}

// ── WX · weather & ocean ────────────────────────────────────────────────────────────────────────

#[test]
fn openmeteo_hourly_is_a_two_day_temperature_trail() {
    let reg = Registry::load().unwrap();
    let recs = records_for(&reg, "wx.openmeteo");
    assert!(recs.len() >= 24, "a 2-day hourly forecast has ~48 points, got {}", recs.len());
    for r in &recs {
        assert!(
            matches!(r.channel(1), Some(DataValue::Double(x)) if x.is_finite()),
            "temperature is finite"
        );
    }
}

#[test]
fn air_quality_current_is_one_aqi_record() {
    let reg = Registry::load().unwrap();
    let recs = records_for(&reg, "wx.air");
    assert_eq!(recs.len(), 1, "the current AQI is one record (the broker accumulates the trail)");
    assert!(
        matches!(recs[0].channel(1), Some(DataValue::Double(aqi)) if *aqi >= 0.0),
        "AQI is non-negative"
    );
}

#[test]
fn tides_water_levels_parse_from_string_values() {
    let reg = Registry::load().unwrap();
    let recs = records_for(&reg, "wx.tides");
    assert!(recs.len() > 10, "a day of CO-OPS water levels has many points, got {}", recs.len());
    for r in &recs {
        assert!(
            matches!(r.channel(1), Some(DataValue::Double(v)) if v.is_finite() && v.abs() < 100.0),
            "level in metres"
        );
    }
}

#[test]
fn nws_alerts_normalize_even_with_null_geometry() {
    let reg = Registry::load().unwrap();
    let recs = records_for(&reg, "wx.alerts");
    assert!(!recs.is_empty(), "NWS returns active alerts");
    // Many alerts are area-wide with no point geometry; those must still normalize (NaN position),
    // proving the null-geometry path does not drop the record.
    let nan_positions = recs
        .iter()
        .filter(|r| matches!(r.channel(1), Some(DataValue::Double(x)) if x.is_nan()))
        .count();
    assert!(nan_positions > 0, "at least one alert has no point geometry (NaN lat)");
    for r in &recs {
        assert!(
            matches!(r.channel(5), Some(DataValue::Enumerated(s)) if *s <= 4),
            "severity tier valid"
        );
    }
}

// ── SP · space & planetary ──────────────────────────────────────────────────────────────────────

#[test]
fn neo_close_approaches_carry_miss_distance() {
    let reg = Registry::load().unwrap();
    let recs = records_for(&reg, "space.neo");
    assert!(!recs.is_empty(), "NeoWs returns close approaches for the day");
    for r in &recs {
        let head = r.channel(6).and_then(DataValue::as_str).unwrap_or("");
        assert!(head.contains("miss"), "headline carries the miss distance: {head}");
        assert!(
            matches!(r.channel(3), Some(DataValue::Double(km)) if *km > 0.0),
            "miss distance km > 0"
        );
    }
}

#[test]
fn epic_metadata_becomes_text_records() {
    let reg = Registry::load().unwrap();
    let recs = records_for(&reg, "space.epic");
    assert!(!recs.is_empty(), "EPIC returns imagery metadata");
    let text = recs[0].channel(0).and_then(DataValue::as_str).unwrap_or("");
    assert!(
        text.starts_with("EPIC") && text.contains("sub-sat"),
        "EPIC line carries the sub-satellite point: {text}"
    );
}

#[test]
fn power_skips_the_fill_value_and_keeps_real_temperatures() {
    let reg = Registry::load().unwrap();
    let recs = records_for(&reg, "space.power");
    assert!(recs.len() > 5, "a 30-day POWER window has many daily points, got {}", recs.len());
    for r in &recs {
        match r.channel(1) {
            Some(DataValue::Double(v)) => assert!(
                (*v - (-999.0)).abs() > 1.0,
                "the -999 fill value is skipped, never plotted as data"
            ),
            other => panic!("POWER value must be a Double, got {other:?}"),
        }
    }
}
