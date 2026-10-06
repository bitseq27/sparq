//! The normalizers: every stream's raw payload → frozen [`DataRecord`]s (plan §6.1, D10).
//!
//! One function per feed, dispatched by registry id, each written against the *recorded* payload
//! shape (the fixtures in `reference/fixtures/observatory/`, captured live by
//! `tools/streams_record.py`) rather than a guessed one — the plan's probe-first discipline, so a
//! normalizer is a specification of a real response, not a promise about one. Everything lands in
//! one of the five [`crate::schema`] shapes on the frozen [`crate::record::DataValue`] carriers; a
//! feed that cannot be expressed is refused in words, never squeezed into the wrong carrier.
//!
//! This module rides the `streams` feature (it is the one place `serde_json` is used). It reads no
//! clock: `now_unix` is injected and `age_s` is computed from it and each record's own `t_utc`, so
//! the same fixture normalizes bit-identically every run (the determinism firewall, plan D11).
//!
//! # Severity is a tier, detail is a headline
//!
//! Event feeds carry wildly different native severities (a USGS magnitude, a GOES flare class, a
//! GDACS alert level, an NWS severity word, a FIRMS confidence letter, an EONET category). Each is
//! mapped onto the shared [`crate::schema`] severity tier (the `enumerated` channel) and the
//! stream-specific detail is kept verbatim in the `headline` text channel — machine-orderable tier
//! plus human-readable words, no colour required (token rule 7).

use serde_json::Value;

use crate::record::DataRecord;
use crate::registry::StreamDef;
use crate::schema::{
    self, SEVERITY_HIGH, SEVERITY_LOW, SEVERITY_MODERATE, SEVERITY_SEVERE, SEVERITY_UNKNOWN,
};
use crate::time;

/// A normalizer failure, in words naming the stream and what went wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizeError {
    /// The stream id whose payload could not be normalized.
    pub stream_id: String,
    /// What went wrong, in words.
    pub message: String,
}

impl std::fmt::Display for NormalizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "normalize `{}`: {}", self.stream_id, self.message)
    }
}

impl std::error::Error for NormalizeError {}

impl NormalizeError {
    fn new(stream_id: &str, message: impl Into<String>) -> Self {
        Self { stream_id: stream_id.to_string(), message: message.into() }
    }
}

/// Normalizes one stream's raw payload into records, given the registry row and the injected fetch
/// time (`now_unix`, for `age_s`). This is the broker's single entry point; it dispatches on the
/// stream id and refuses an un-normalized stream in words.
///
/// # Errors
/// A [`NormalizeError`] when the payload is not the shape the feed promised (a JSON parse failure, a
/// missing required field), or when no normalizer exists for the id — both in words, never a panic.
pub fn normalize(
    def: &StreamDef,
    body: &str,
    now_unix: i64,
) -> Result<Vec<DataRecord>, NormalizeError> {
    let id = def.id.as_str();
    let e = |m: String| NormalizeError::new(id, m);
    match id {
        "swpc.aurora" => aurora(body, now_unix)
            .map_err(|_| e("aurora payload is not the OVATION JSON shape".into())),
        "swpc.kp" => summary_timeseries(body, now_unix, "Kp")
            .map_err(|_| e("Kp payload is not a SWPC summary array".into())),
        "swpc.solar-wind" => summary_timeseries(body, now_unix, "proton_speed")
            .map_err(|_| e("solar-wind payload is not a SWPC summary array".into())),
        "swpc.bz" => summary_timeseries(body, now_unix, "bz_gsm")
            .map_err(|_| e("Bz payload is not a SWPC summary array".into())),
        "swpc.flux10cm" => summary_timeseries(body, now_unix, "flux")
            .map_err(|_| e("10cm-flux payload is not a SWPC summary array".into())),
        "swpc.xray-flares" => xray_flares(body, now_unix)
            .map_err(|_| e("flare payload is not the GOES xray-flares array".into())),
        "swpc.wwv" | "swpc.geomag-forecast" => Ok(raw_text(body, now_unix)),
        "sat.iss" => iss(body, now_unix)
            .map_err(|_| e("ISS payload is not the wheretheiss.at object".into())),
        "sat.tle-stations" => tle_table(body, now_unix)
            .map_err(|_| e("TLE payload is not the CelesTrak GP array".into())),
        "geo.quakes-hour" | "geo.quakes-day" => {
            usgs_quakes(body, now_unix).map_err(|_| e("quake payload is not USGS GeoJSON".into()))
        },
        "geo.gdacs" => gdacs(body, now_unix)
            .map_err(|_| e("GDACS payload is not a GeoJSON FeatureCollection".into())),
        "geo.eonet" | "geo.wildfires" => {
            eonet(body, now_unix).map_err(|_| e("EONET payload is not the v3 events object".into()))
        },
        "geo.firms" => Ok(firms_csv(body, now_unix)),
        "wx.openmeteo" => openmeteo(body, now_unix)
            .map_err(|_| e("open-meteo payload is not the forecast object".into())),
        "wx.air" => air(body, now_unix)
            .map_err(|_| e("air-quality payload is not the open-meteo object".into())),
        "wx.tides" => tides(body, now_unix)
            .map_err(|_| e("CO-OPS payload is not the datagetter object".into())),
        "wx.alerts" => nws_alerts(body, now_unix)
            .map_err(|_| e("NWS payload is not a GeoJSON FeatureCollection".into())),
        "space.neo" => {
            neo(body, now_unix).map_err(|_| e("NEO payload is not the NeoWs feed object".into()))
        },
        "space.epic" => Ok(epic(body, now_unix)),
        "space.power" => power(body, now_unix)
            .map_err(|_| e("POWER payload is not the daily point object".into())),
        other => Err(NormalizeError::new(
            other,
            "no normalizer for this stream id — the registry knows it, so normalize.rs must too \
             (add a per-stream function and a dispatch arm; plan §6.1)",
        )),
    }
}

/// True when `id` has a normalizer here. The registry-drift test asserts every registry id does, so
/// "add a stream" can never silently forget the normalizer half.
#[must_use]
pub fn has_normalizer(id: &str) -> bool {
    matches!(
        id,
        "swpc.aurora"
            | "swpc.kp"
            | "swpc.solar-wind"
            | "swpc.bz"
            | "swpc.flux10cm"
            | "swpc.xray-flares"
            | "swpc.wwv"
            | "swpc.geomag-forecast"
            | "sat.iss"
            | "sat.tle-stations"
            | "geo.quakes-hour"
            | "geo.quakes-day"
            | "geo.gdacs"
            | "geo.eonet"
            | "geo.wildfires"
            | "geo.firms"
            | "wx.openmeteo"
            | "wx.air"
            | "wx.tides"
            | "wx.alerts"
            | "space.neo"
            | "space.epic"
            | "space.power"
    )
}

// --- shared helpers -----------------------------------------------------------------------------

/// `age_s` from the injected clock and a record's own time: never negative (a forecast record dated
/// after the fetch reads as age 0, not a negative age).
fn age(now_unix: i64, t_utc: i64) -> f64 {
    now_unix.saturating_sub(t_utc).max(0) as f64
}

/// Parses JSON, mapping the serde error into words.
fn json(body: &str) -> Result<Value, String> {
    serde_json::from_str::<Value>(body).map_err(|e| format!("JSON parse failed: {e}"))
}

/// Reads a number from a JSON value that may carry it as a JSON number OR a numeric string (NEO's
/// miss distance, CO-OPS' water level). Never guesses: a non-numeric string is `None`.
fn num(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse::<f64>().ok()))
}

/// Like [`num`] but for integers (unix-ms stamps that ride as JSON numbers).
fn integer(v: &Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_f64().map(|f| f as i64))
        .or_else(|| v.as_str().and_then(|s| s.trim().parse::<i64>().ok()))
}

/// The first `[lon, lat]` pair in a GeoJSON `coordinates` value, descending through Point /
/// LineString / Polygon / Multi* nesting. `None` when there is no coordinate pair (a null geometry).
fn first_coord(v: &Value) -> Option<(f64, f64)> {
    let arr = v.as_array()?;
    if arr.len() >= 2 {
        if let (Some(a), Some(b)) = (num(&arr[0]), num(&arr[1])) {
            return Some((a, b));
        }
    }
    arr.first().and_then(first_coord)
}

/// Sorts records oldest → newest by their wall stamp, so a timeseries window reads in time order
/// regardless of the feed's own ordering.
fn sort_by_time(records: &mut [DataRecord]) {
    records.sort_by_key(|r| r.t_wall_ns);
}

// --- severity tier mappings (native vocabularies -> the shared scale) ---------------------------

/// USGS magnitude → tier (schema.rs's documented bands).
fn mag_severity(mag: f64) -> u32 {
    if !mag.is_finite() || mag < 0.0 {
        SEVERITY_UNKNOWN
    } else if mag < 2.5 {
        SEVERITY_LOW
    } else if mag < 4.5 {
        SEVERITY_MODERATE
    } else if mag < 6.0 {
        SEVERITY_HIGH
    } else {
        SEVERITY_SEVERE
    }
}

/// GOES flare class letter → tier (A/B/C/M/X; A and B are the weakest real classes).
fn goes_severity(class: &str) -> u32 {
    match class.trim().chars().next().unwrap_or('?').to_ascii_uppercase() {
        'A' | 'B' => SEVERITY_LOW,
        'C' => SEVERITY_MODERATE,
        'M' => SEVERITY_HIGH,
        'X' => SEVERITY_SEVERE,
        _ => SEVERITY_UNKNOWN,
    }
}

/// GDACS alert level → tier.
fn gdacs_severity(level: &str) -> u32 {
    match level.trim().to_ascii_lowercase().as_str() {
        "green" => SEVERITY_LOW,
        "orange" => SEVERITY_MODERATE,
        "red" => SEVERITY_HIGH,
        _ => SEVERITY_UNKNOWN,
    }
}

/// NWS alert severity word → tier.
fn nws_severity(level: &str) -> u32 {
    match level.trim().to_ascii_lowercase().as_str() {
        "minor" => SEVERITY_LOW,
        "moderate" => SEVERITY_MODERATE,
        "severe" => SEVERITY_HIGH,
        "extreme" => SEVERITY_SEVERE,
        _ => SEVERITY_UNKNOWN,
    }
}

/// FIRMS detection confidence letter → tier.
fn confidence_severity(c: &str) -> u32 {
    match c.trim().to_ascii_lowercase().as_str() {
        "l" => SEVERITY_LOW,
        "n" => SEVERITY_LOW,
        "nom" | "m" => SEVERITY_MODERATE,
        "h" => SEVERITY_HIGH,
        _ => SEVERITY_UNKNOWN,
    }
}

/// EONET category id → a best-effort tier (the category itself rides the headline; this only orders
/// the dots by rough urgency for the map renderer).
fn eonet_severity(category: &str) -> u32 {
    match category {
        "volcanoes" | "severeStorms" => SEVERITY_HIGH,
        "wildfires" | "landslides" | "snowstorms" | "floods" => SEVERITY_MODERATE,
        _ => SEVERITY_LOW,
    }
}

// --- per-stream normalizers ---------------------------------------------------------------------

/// OVATION aurora: ~65 k `[lon, lat, intensity]` triples at 1°, regridded broker-side to a 2° grid
/// (180 × 90 = 16 200 cells, plan §6.4) so the guest never holds the 925 KB payload. Empty cells
/// are NaN (the contract's no-cell sentinel). One grid record per fetch.
fn aurora(body: &str, now_unix: i64) -> Result<Vec<DataRecord>, String> {
    let v = json(body)?;
    let coords = v.get("coordinates").and_then(Value::as_array).ok_or("no `coordinates` array")?;
    const COLS: usize = 180; // 360° / 2°
    const ROWS: usize = 90; // 180° / 2°
    let mut sum = vec![0.0f64; COLS * ROWS];
    let mut count = vec![0u32; COLS * ROWS];
    for triple in coords {
        let row = triple.as_array();
        if let Some([lon_v, lat_v, val_v]) = row.map(|r| r.as_slice()) {
            if let (Some(mut lon), Some(lat), Some(val)) = (num(lon_v), num(lat_v), num(val_v)) {
                if lon > 180.0 {
                    lon -= 360.0; // OVATION reports 0..359; fold to -180..180
                }
                let c =
                    (((lon + 180.0) / 2.0).floor() as isize).clamp(0, (COLS - 1) as isize) as usize;
                let r =
                    (((lat + 90.0) / 2.0).floor() as isize).clamp(0, (ROWS - 1) as isize) as usize;
                let i = r * COLS + c;
                sum[i] += val;
                count[i] += 1;
            }
        }
    }
    let values: Vec<f64> = (0..COLS * ROWS)
        .map(|i| if count[i] > 0 { sum[i] / f64::from(count[i]) } else { f64::NAN })
        .collect();
    let t_utc = v
        .get("Forecast Time")
        .or_else(|| v.get("Observation Time"))
        .and_then(|s| s.as_str())
        .and_then(time::parse_timestamp)
        .unwrap_or(now_unix);
    Ok(vec![schema::grid(
        COLS as i32,
        ROWS as i32,
        -90.0,  // lat0: row 0 is the southern edge
        -180.0, // lon0: col 0 is the western edge
        2.0,    // step_deg
        values,
        t_utc,
        age(now_unix, t_utc),
    )])
}

/// The SWPC `products/summary/*` family: a one-or-more-element array of `{time_tag, <field>}`. One
/// timeseries record per element (these endpoints report the latest reading; the broker accumulates
/// the trail across polls).
fn summary_timeseries(body: &str, now_unix: i64, field: &str) -> Result<Vec<DataRecord>, String> {
    let v = json(body)?;
    let arr = v.as_array().ok_or("payload is not an array")?;
    let mut out = Vec::new();
    for o in arr {
        let t_utc = o.get("time_tag").and_then(|s| s.as_str()).and_then(time::parse_timestamp);
        let value = o.get(field).and_then(num);
        if let (Some(t), Some(val)) = (t_utc, value) {
            out.push(schema::timeseries(t, val, age(now_unix, t)));
        }
    }
    sort_by_time(&mut out);
    Ok(out)
}

/// GOES X-ray flares (7-day): each `{begin_time, begin_class, max_class, …}` → an event record. The
/// class letter sets the severity tier, the numeric part rides `magnitude`, the full class rides the
/// headline. Flares have no place, so lat/lon are NaN (the map renderer skips them; the timeline
/// scatter plots class-vs-time).
fn xray_flares(body: &str, now_unix: i64) -> Result<Vec<DataRecord>, String> {
    let v = json(body)?;
    let arr = v.as_array().ok_or("payload is not an array")?;
    let mut out = Vec::new();
    for o in arr {
        let t_utc = o
            .get("begin_time")
            .or_else(|| o.get("time_tag"))
            .and_then(|s| s.as_str())
            .and_then(time::parse_timestamp)
            .unwrap_or(now_unix);
        // Null-safe fallback: `max_class` may be present-but-null for an in-progress flare, and a
        // JSON null must fall through to `begin_class` exactly like an absent key (not read as "?").
        let class = o
            .get("max_class")
            .and_then(Value::as_str)
            .or_else(|| o.get("begin_class").and_then(Value::as_str))
            .unwrap_or("?");
        let numeric = class[1..].trim().parse::<f64>().unwrap_or(f64::NAN);
        out.push(schema::event(
            t_utc,
            f64::NAN,
            f64::NAN,
            numeric,
            0.0,
            goes_severity(class),
            &format!("GOES {class} X-ray flare"),
            age(now_unix, t_utc),
        ));
    }
    sort_by_time(&mut out);
    Ok(out)
}

/// A raw-text bulletin (WWV, 3-day geomag forecast): the payload verbatim in the `text` channel —
/// the ticker shows the raw feed as words (plan D8). The issue time comes from the `:Issued:` header
/// when present, else the fetch time (honestly, never an invented date).
fn raw_text(body: &str, now_unix: i64) -> Vec<DataRecord> {
    let issued = time::parse_issued_header(body).unwrap_or(now_unix);
    vec![schema::text(body, issued, age(now_unix, issued))]
}

/// wheretheiss.at ISS position. Note the unit conversion: the API reports velocity in km/h, the
/// frozen position schema wants km/s (`vel_kms`), so we divide by 3600 — a unit rule, not a taste.
fn iss(body: &str, now_unix: i64) -> Result<Vec<DataRecord>, String> {
    let v = json(body)?;
    let lat = v.get("latitude").and_then(num).ok_or("no latitude")?;
    let lon = v.get("longitude").and_then(num).ok_or("no longitude")?;
    let alt = v.get("altitude").and_then(num).unwrap_or(f64::NAN);
    let vel_kmh = v.get("velocity").and_then(num).unwrap_or(f64::NAN);
    let footprint = v.get("footprint").and_then(num).unwrap_or(f64::NAN);
    let vis = v
        .get("visibility")
        .and_then(Value::as_str)
        .map(schema::visibility_from_str)
        .unwrap_or(schema::VISIBILITY_UNKNOWN);
    let t_utc = v.get("timestamp").and_then(integer).unwrap_or(now_unix);
    Ok(vec![schema::position(
        lat,
        lon,
        alt,
        vel_kmh / 3600.0,
        footprint,
        vis,
        t_utc,
        age(now_unix, t_utc),
    )])
}

/// CelesTrak "stations" GP catalogue → one text record holding a compact table (the plan's "TLE
/// table"; SGP4 ground tracks are INC6). Each line: name, epoch, inclination, mean motion.
fn tle_table(body: &str, now_unix: i64) -> Result<Vec<DataRecord>, String> {
    let v = json(body)?;
    let arr = v.as_array().ok_or("payload is not an array")?;
    let mut lines: Vec<String> = Vec::with_capacity(arr.len());
    for o in arr {
        let name = o.get("OBJECT_NAME").and_then(Value::as_str).unwrap_or("?");
        let epoch = o.get("EPOCH").and_then(Value::as_str).unwrap_or("?");
        let inc = o.get("INCLINATION").and_then(num).unwrap_or(f64::NAN);
        let mm = o.get("MEAN_MOTION").and_then(num).unwrap_or(f64::NAN);
        lines.push(format!("{name} | epoch {epoch} | inc {inc:.2}° | {mm:.4} rev/day"));
    }
    let table = lines.join("\n");
    Ok(vec![schema::text(&table, now_unix, 0.0)])
}

/// USGS earthquake GeoJSON: each feature → an event. `time` is unix **milliseconds**; coordinates
/// are `[lon, lat, depth_km]`; magnitude and place ride the properties.
fn usgs_quakes(body: &str, now_unix: i64) -> Result<Vec<DataRecord>, String> {
    let v = json(body)?;
    let feats = v.get("features").and_then(Value::as_array).ok_or("no `features`")?;
    let mut out = Vec::with_capacity(feats.len());
    for f in feats {
        let p = f.get("properties").cloned().unwrap_or(Value::Null);
        let t_ms = p.get("time").and_then(integer).unwrap_or(now_unix * 1000);
        let t_utc = t_ms / 1000;
        let (lon, lat, depth) = match f
            .get("geometry")
            .and_then(|g| g.get("coordinates"))
            .and_then(first_coord_and_depth)
        {
            Some((lon, lat, depth)) => (lon, lat, depth),
            None => (f64::NAN, f64::NAN, f64::NAN),
        };
        let mag = p.get("mag").and_then(num).unwrap_or(f64::NAN);
        let place = p.get("place").and_then(Value::as_str).unwrap_or("unknown location");
        let headline = if mag.is_finite() {
            format!("M {mag:.1} — {place}")
        } else {
            format!("M ? — {place}")
        };
        out.push(schema::event(
            t_utc,
            lat,
            lon,
            mag,
            depth,
            mag_severity(mag),
            &headline,
            age(now_unix, t_utc),
        ));
    }
    sort_by_time(&mut out);
    Ok(out)
}

/// Like [`first_coord`] but also returns the third element (depth) when present, for USGS quakes.
fn first_coord_and_depth(v: &Value) -> Option<(f64, f64, f64)> {
    let arr = v.as_array()?;
    if arr.len() >= 2 {
        if let (Some(lon), Some(lat)) = (num(&arr[0]), num(&arr[1])) {
            let depth = arr.get(2).and_then(num).unwrap_or(0.0);
            return Some((lon, lat, depth));
        }
    }
    arr.first().and_then(first_coord_and_depth)
}

/// GDACS EQ/TC/FL GeoJSON: each feature → an event. Severity from the alert level, headline from the
/// event name, time from `fromdate`, place from the point geometry.
fn gdacs(body: &str, now_unix: i64) -> Result<Vec<DataRecord>, String> {
    let v = json(body)?;
    let feats = v.get("features").and_then(Value::as_array).ok_or("no `features`")?;
    let mut out = Vec::with_capacity(feats.len());
    for f in feats {
        let p = f.get("properties").cloned().unwrap_or(Value::Null);
        let t_utc = p
            .get("fromdate")
            .and_then(Value::as_str)
            .and_then(time::parse_timestamp)
            .unwrap_or(now_unix);
        let (lon, lat) = f
            .get("geometry")
            .and_then(|g| g.get("coordinates"))
            .and_then(first_coord)
            .unwrap_or((f64::NAN, f64::NAN));
        let level = p.get("alertlevel").and_then(Value::as_str).unwrap_or("");
        let etype = p.get("eventtype").and_then(Value::as_str).unwrap_or("");
        let name = p.get("name").and_then(Value::as_str).unwrap_or("GDACS event");
        let country = p.get("country").and_then(Value::as_str).unwrap_or("");
        let headline = if country.is_empty() {
            format!("{etype}: {name}")
        } else {
            format!("{etype}: {name} ({country})")
        };
        out.push(schema::event(
            t_utc,
            lat,
            lon,
            0.0,
            0.0,
            gdacs_severity(level),
            &headline,
            age(now_unix, t_utc),
        ));
    }
    sort_by_time(&mut out);
    Ok(out)
}

/// NASA EONET v3 events: each event's first geometry gives time + place + magnitude; the category
/// rides the headline (the bars renderer bins on it). Point and Polygon geometries both resolve via
/// [`first_coord`].
fn eonet(body: &str, now_unix: i64) -> Result<Vec<DataRecord>, String> {
    let v = json(body)?;
    let events = v.get("events").and_then(Value::as_array).ok_or("no `events`")?;
    let mut out = Vec::with_capacity(events.len());
    for ev in events {
        let title = ev.get("title").and_then(Value::as_str).unwrap_or("EONET event");
        let category = ev
            .get("categories")
            .and_then(Value::as_array)
            .and_then(|c| c.first())
            .and_then(|c| c.get("id").and_then(Value::as_str))
            .unwrap_or("unknown");
        let geom = ev
            .get("geometry")
            .and_then(Value::as_array)
            .and_then(|g| g.first())
            .cloned()
            .unwrap_or(Value::Null);
        let t_utc = geom
            .get("date")
            .and_then(Value::as_str)
            .and_then(time::parse_timestamp)
            .unwrap_or(now_unix);
        let (lon, lat) =
            geom.get("coordinates").and_then(first_coord).unwrap_or((f64::NAN, f64::NAN));
        let magnitude = geom.get("magnitudeValue").and_then(num).unwrap_or(f64::NAN);
        let headline = format!("{category}: {title}");
        out.push(schema::event(
            t_utc,
            lat,
            lon,
            magnitude,
            0.0,
            eonet_severity(category),
            &headline,
            age(now_unix, t_utc),
        ));
    }
    sort_by_time(&mut out);
    Ok(out)
}

/// NASA FIRMS active-fire CSV (the synthetic sample when no key is set; the live feed with a free
/// key). Column order per the FIRMS VIIRS CSV API; `acq_date` + `acq_time` (HHMM) make the stamp.
fn firms_csv(body: &str, now_unix: i64) -> Vec<DataRecord> {
    let mut lines = body.lines().filter(|l| !l.trim().is_empty());
    let header = match lines.next() {
        Some(h) => h,
        None => return Vec::new(),
    };
    let cols: Vec<&str> = header.split(',').map(|c| c.trim()).collect();
    let idx = |name: &str| cols.iter().position(|c| c.eq_ignore_ascii_case(name));
    let (Some(i_lat), Some(i_lon)) = (idx("latitude"), idx("longitude")) else {
        return Vec::new(); // not a FIRMS header — no rows, no guess
    };
    let i_bright = idx("brightness");
    let i_date = idx("acq_date");
    let i_time = idx("acq_time");
    let i_conf = idx("confidence");
    let i_frp = idx("frp");
    let mut out = Vec::new();
    for line in lines {
        let f: Vec<&str> = line.split(',').map(|c| c.trim()).collect();
        let at = |i: Option<usize>| i.and_then(|i| f.get(i).copied());
        let (Some(lat), Some(lon)) = (
            at(Some(i_lat)).and_then(|s| s.parse::<f64>().ok()),
            at(Some(i_lon)).and_then(|s| s.parse::<f64>().ok()),
        ) else {
            continue;
        };
        let date = at(i_date).unwrap_or("");
        let hhmm = at(i_time).unwrap_or("0000");
        let stamp = if hhmm.len() == 4 {
            format!("{date}T{}:{}", &hhmm[0..2], &hhmm[2..4])
        } else {
            format!("{date}T00:00")
        };
        let t_utc = time::parse_timestamp(&stamp).unwrap_or(now_unix);
        let bright = at(i_bright).and_then(|s| s.parse::<f64>().ok()).unwrap_or(f64::NAN);
        let frp = at(i_frp).and_then(|s| s.parse::<f64>().ok()).unwrap_or(f64::NAN);
        let conf = at(i_conf).unwrap_or("");
        out.push(schema::event(
            t_utc,
            lat,
            lon,
            frp,
            0.0,
            confidence_severity(conf),
            &format!("FIRMS fire {lat:.3},{lon:.3} · {bright:.0} K · FRP {frp:.1}"),
            age(now_unix, t_utc),
        ));
    }
    sort_by_time(&mut out);
    out
}

/// Open-Meteo forecast: the hourly temperature series → timeseries records (the trail the cell's
/// trace draws; the gauge reads the latest). `value` is `temperature_2m` in °C.
fn openmeteo(body: &str, now_unix: i64) -> Result<Vec<DataRecord>, String> {
    let v = json(body)?;
    let hourly = v.get("hourly").ok_or("no `hourly`")?;
    let times = hourly.get("time").and_then(Value::as_array).ok_or("no hourly.time")?;
    let temps =
        hourly.get("temperature_2m").and_then(Value::as_array).ok_or("no hourly.temperature_2m")?;
    let mut out = Vec::with_capacity(times.len().min(temps.len()));
    for (t, val) in times.iter().zip(temps.iter()) {
        if let (Some(ts), Some(x)) = (t.as_str().and_then(time::parse_timestamp), num(val)) {
            out.push(schema::timeseries(ts, x, age(now_unix, ts)));
        }
    }
    sort_by_time(&mut out);
    Ok(out)
}

/// Open-Meteo air quality: the current US AQI → one timeseries record (the gauge's needle; the
/// broker accumulates the trail across polls).
fn air(body: &str, now_unix: i64) -> Result<Vec<DataRecord>, String> {
    let v = json(body)?;
    let cur = v.get("current").ok_or("no `current`")?;
    let aqi = cur
        .get("us_aqi")
        .and_then(num)
        .or_else(|| cur.get("pm2_5").and_then(num))
        .ok_or("no us_aqi/pm2_5")?;
    let t_utc =
        cur.get("time").and_then(Value::as_str).and_then(time::parse_timestamp).unwrap_or(now_unix);
    Ok(vec![schema::timeseries(t_utc, aqi, age(now_unix, t_utc))])
}

/// NOAA CO-OPS water level: each `{t, v}` (v is a **string** in metres) → a timeseries record.
fn tides(body: &str, now_unix: i64) -> Result<Vec<DataRecord>, String> {
    let v = json(body)?;
    let data = v.get("data").and_then(Value::as_array).ok_or("no `data`")?;
    let mut out = Vec::with_capacity(data.len());
    for o in data {
        let t_utc = o.get("t").and_then(Value::as_str).and_then(time::parse_timestamp);
        let level = o.get("v").and_then(num);
        if let (Some(t), Some(x)) = (t_utc, level) {
            out.push(schema::timeseries(t, x, age(now_unix, t)));
        }
    }
    sort_by_time(&mut out);
    Ok(out)
}

/// NWS active alerts GeoJSON: each feature → an event. Severity from the NWS word, time from
/// `effective`, headline from `headline` or `event`+`areaDesc`. Many alerts are area-wide with a
/// null geometry — those get NaN lat/lon (the map skips them; the ticker still reads them).
fn nws_alerts(body: &str, now_unix: i64) -> Result<Vec<DataRecord>, String> {
    let v = json(body)?;
    let feats = v.get("features").and_then(Value::as_array).ok_or("no `features`")?;
    let mut out = Vec::with_capacity(feats.len());
    for f in feats {
        let p = f.get("properties").cloned().unwrap_or(Value::Null);
        let t_utc = p
            .get("effective")
            .and_then(Value::as_str)
            .and_then(time::parse_timestamp)
            .unwrap_or(now_unix);
        let sev = p.get("severity").and_then(Value::as_str).unwrap_or("Unknown");
        let event = p.get("event").and_then(Value::as_str).unwrap_or("Alert");
        let area = p.get("areaDesc").and_then(Value::as_str).unwrap_or("");
        let headline = match p.get("headline").and_then(Value::as_str) {
            Some(h) if !h.is_empty() => h.to_string(),
            _ => {
                if area.is_empty() {
                    event.to_string()
                } else {
                    format!("{event} — {area}")
                }
            },
        };
        let (lon, lat) = f
            .get("geometry")
            .and_then(|g| g.get("coordinates"))
            .and_then(first_coord)
            .unwrap_or((f64::NAN, f64::NAN));
        out.push(schema::event(
            t_utc,
            lat,
            lon,
            0.0,
            0.0,
            nws_severity(sev),
            &headline,
            age(now_unix, t_utc),
        ));
    }
    sort_by_time(&mut out);
    Ok(out)
}

/// NASA NeoWs close approaches: each object's close-approach → an event on the miss-distance
/// timeline. `epoch_date_close_approach` is unix **ms**; the miss distance rides as a numeric string
/// in km; hazardous objects get the HIGH tier. No place (a close approach is not a ground point), so
/// lat/lon are NaN.
fn neo(body: &str, now_unix: i64) -> Result<Vec<DataRecord>, String> {
    let v = json(body)?;
    let neo_by_day =
        v.get("near_earth_objects").and_then(Value::as_object).ok_or("no `near_earth_objects`")?;
    let mut out = Vec::new();
    for (_day, objs) in neo_by_day {
        let Some(objs) = objs.as_array() else { continue };
        for o in objs {
            let name = o.get("name").and_then(Value::as_str).unwrap_or("NEO");
            let hazardous = o
                .get("is_potentially_hazardous_asteroid")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let approaches =
                o.get("close_approach_data").and_then(Value::as_array).cloned().unwrap_or_default();
            for ca in &approaches {
                let t_utc = ca
                    .get("epoch_date_close_approach")
                    .and_then(integer)
                    .map(|ms| ms / 1000)
                    .unwrap_or(now_unix);
                let miss_km = ca
                    .get("miss_distance")
                    .and_then(|m| m.get("kilometers"))
                    .and_then(num)
                    .unwrap_or(f64::NAN);
                let sev = if hazardous { SEVERITY_HIGH } else { SEVERITY_LOW };
                let headline = if miss_km.is_finite() {
                    format!(
                        "{name} — miss {} km{}",
                        {
                            let mut s = format!("{miss_km:.0}");
                            insert_thousands(&mut s);
                            s
                        },
                        if hazardous { " (hazardous)" } else { "" }
                    )
                } else {
                    format!("{name} — close approach")
                };
                out.push(schema::event(
                    t_utc,
                    f64::NAN,
                    f64::NAN,
                    miss_km,
                    0.0,
                    sev,
                    &headline,
                    age(now_unix, t_utc),
                ));
            }
        }
    }
    sort_by_time(&mut out);
    Ok(out)
}

/// Inserts thousands separators into an integer string, in place (a tiny formatter so the headline
/// reads "74,094,317 km" without pulling a formatting crate).
fn insert_thousands(s: &mut String) {
    let digits: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*c);
    }
    *s = out;
}

/// NASA EPIC (DSCOVR) imagery **metadata** (the imagery itself is pixels, out of scope, plan §13):
/// each entry → a text record with its capture stamp and sub-satellite point, for the ticker.
fn epic(body: &str, now_unix: i64) -> Vec<DataRecord> {
    let Ok(v) = json(body) else { return Vec::new() };
    let Some(arr) = v.as_array() else { return Vec::new() };
    let mut out = Vec::with_capacity(arr.len());
    for o in arr {
        let ident = o.get("identifier").and_then(Value::as_str).unwrap_or("");
        let date = o.get("date").and_then(Value::as_str);
        let t_utc = date
            .and_then(time::parse_timestamp)
            .or_else(|| time::parse_timestamp(ident))
            .unwrap_or(now_unix);
        let lat = o
            .get("centroid_coordinates")
            .and_then(|c| c.get("lat"))
            .and_then(num)
            .unwrap_or(f64::NAN);
        let lon = o
            .get("centroid_coordinates")
            .and_then(|c| c.get("lon"))
            .and_then(num)
            .unwrap_or(f64::NAN);
        let line = format!("EPIC {ident} · sub-sat {lat:.2},{lon:.2}");
        out.push(schema::text(&line, t_utc, age(now_unix, t_utc)));
    }
    out
}

/// NASA POWER daily point: `properties.parameter.T2M` is a `{YYYYMMDD: value}` map (fill value
/// -999 = missing). One timeseries record per valid day; the 2–3 day publication lag is why the cell
/// footer labels POWER NEAR-REALTIME (plan §3.1 row 23).
fn power(body: &str, now_unix: i64) -> Result<Vec<DataRecord>, String> {
    let v = json(body)?;
    let fill = v.get("header").and_then(|h| h.get("fill_value")).and_then(num).unwrap_or(-999.0);
    let t2m = v
        .get("properties")
        .and_then(|p| p.get("parameter"))
        .and_then(|p| p.get("T2M"))
        .and_then(Value::as_object)
        .ok_or("no properties.parameter.T2M")?;
    let mut out = Vec::with_capacity(t2m.len());
    for (date, val) in t2m {
        let Some(x) = num(val) else { continue };
        if (x - fill).abs() < f64::EPSILON {
            continue; // fill value = missing, never a real 0
        }
        let Some(t_utc) = time::parse_timestamp(date) else { continue };
        out.push(schema::timeseries(t_utc, x, age(now_unix, t_utc)));
    }
    sort_by_time(&mut out);
    Ok(out)
}
