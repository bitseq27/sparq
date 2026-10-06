//! Timestamp parsing for the stream feeds — dependency-free, clock-free, total.
//!
//! Every feed stamps its records differently: ISO-8601 with and without a `Z`, with a `+00:00`
//! offset, with fractional seconds, with a space instead of `T`; compact `YYYYMMDD` (NASA POWER) and
//! `YYYYMMDDHHMMSS`; a space-separated `YYYY-MM-DD HH:MM` (NOAA CO-OPS); SWPC's `:Issued:` line
//! (`2026 Oct 06 0005 UTC`); and plain unix seconds/milliseconds the normalizers divide themselves.
//! This module turns the *textual* forms into unix seconds so the frozen schemas' `t_utc` channel is
//! always a real integer, and it does so with no `chrono`, no `SystemTime` (both would break the
//! zero-dep core and the determinism firewall, plan D11) and no panics: an unparseable stamp is
//! `None`, and the caller decides whether that is fatal (it usually falls back to the fetch time,
//! honestly labelled, rather than inventing a date).
//!
//! The civil-calendar maths is Howard Hinnant's `days_from_civil` — the standard proleptic-Gregorian
//! algorithm — reproduced here (a dozen lines) rather than taken as a dependency, in the house's
//! zero-dep idiom.

/// Days from the civil date (proleptic Gregorian) to the Unix epoch day (1970-01-01 = 0).
/// Howard Hinnant's `days_from_civil`; `m` is 1-12, `d` is 1-31.
#[must_use]
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = (m + 9) % 12; // Mar = 0 .. Feb = 11
    let doy = (153 * mp + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

/// Unix seconds for a UTC calendar date/time. Out-of-range fields are still computed (the algorithm
/// is proleptic); callers pass validated components.
#[must_use]
pub fn unix_from_ymdhms(y: i64, mo: i64, d: i64, h: i64, mi: i64, s: i64) -> i64 {
    days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + s
}

/// The three-letter month abbreviations the SWPC `:Issued:` line uses, mapped to 1-12.
fn month_abbr(s: &str) -> Option<i64> {
    Some(match s.to_ascii_lowercase().as_str() {
        "jan" => 1,
        "feb" => 2,
        "mar" => 3,
        "apr" => 4,
        "may" => 5,
        "jun" => 6,
        "jul" => 7,
        "aug" => 8,
        "sep" => 9,
        "oct" => 10,
        "nov" => 11,
        "dec" => 12,
        _ => return None,
    })
}

/// Parses a compact all-digits stamp: `YYYYMMDD` (8), `YYYYMMDDHHMM` (12) or `YYYYMMDDHHMMSS` (14).
fn parse_compact(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    let n = |i: usize| -> Option<i64> {
        let c = b.get(i)?;
        if c.is_ascii_digit() {
            Some(i64::from(c - b'0'))
        } else {
            None
        }
    };
    let num = |from: usize, len: usize| -> Option<i64> {
        let mut v = 0i64;
        for i in from..from + len {
            v = v * 10 + n(i)?;
        }
        Some(v)
    };
    match b.len() {
        8 => {
            let (y, mo, d) = (num(0, 4)?, num(4, 2)?, num(6, 2)?);
            Some(unix_from_ymdhms(y, mo, d, 0, 0, 0))
        },
        12 => {
            let (y, mo, d, h, mi) = (num(0, 4)?, num(4, 2)?, num(6, 2)?, num(8, 2)?, num(10, 2)?);
            Some(unix_from_ymdhms(y, mo, d, h, mi, 0))
        },
        14 => {
            let (y, mo, d, h, mi, s) =
                (num(0, 4)?, num(4, 2)?, num(6, 2)?, num(8, 2)?, num(10, 2)?, num(12, 2)?);
            Some(unix_from_ymdhms(y, mo, d, h, mi, s))
        },
        _ => None,
    }
}

/// Parses SWPC's `:Issued:` stamp: `2026 Oct 06 0005 UTC` (year, month-abbr, day, HHMM, zone).
fn parse_swpc_issued(s: &str) -> Option<i64> {
    let t: Vec<&str> = s.split_whitespace().collect();
    if t.len() < 4 {
        return None;
    }
    let y: i64 = t[0].parse().ok()?;
    let mo = month_abbr(t[1])?;
    let d: i64 = t[2].parse().ok()?;
    let hhmm = t[3];
    if hhmm.len() != 4 || !hhmm.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let h: i64 = hhmm[0..2].parse().ok()?;
    let mi: i64 = hhmm[2..4].parse().ok()?;
    Some(unix_from_ymdhms(y, mo, d, h, mi, 0))
}

/// Parses the ISO-8601 family: `YYYY-MM-DD`, with an optional time after `T` or a space
/// (`HH:MM[:SS][.frac]`), and an optional zone (`Z`, or `±HH:MM` / `±HHMM`). The offset is applied
/// so the result is always UTC unix seconds. Returns `None` on any malformed component.
fn parse_iso(s: &str) -> Option<i64> {
    let s = s.trim();
    // Split the date from the time on 'T' or the first space (a space-separated zone like "… UTC"
    // is not this family; SWPC's issued line is handled separately).
    let (date_part, time_part) = match s.find('T').or_else(|| s.find(' ')) {
        Some(i) => (&s[..i], Some(&s[i + 1..])),
        None => (s, None),
    };
    let mut dp = date_part.split('-');
    let y: i64 = dp.next()?.parse().ok()?;
    let mo: i64 = dp.next()?.parse().ok()?;
    let d: i64 = dp.next()?.parse().ok()?;
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }

    let mut h = 0i64;
    let mut mi = 0i64;
    let mut sec = 0i64;
    let mut offset_s = 0i64;
    if let Some(tp) = time_part {
        let tp = tp.trim();
        // Peel the zone off the end: 'Z', or '±HH:MM' / '±HHMM'.
        let (clock, zone) =
            if let Some(stripped) = tp.strip_suffix('Z').or_else(|| tp.strip_suffix('z')) {
                (stripped, 0i64)
            } else if let Some(sign_i) = tp.rfind(['+', '-']) {
                // A sign at index 0 would be a negative year, not a zone; require it after the clock.
                if sign_i == 0 {
                    (tp, 0i64)
                } else {
                    let (c, z) = tp.split_at(sign_i);
                    (c, parse_zone_offset(z)?)
                }
            } else {
                (tp, 0i64)
            };
        offset_s = zone;
        // clock = "HH:MM[:SS][.frac]"
        let clock = match clock.find('.') {
            Some(dot) => &clock[..dot], // drop fractional seconds
            None => clock,
        };
        let mut parts = clock.split(':');
        h = parts.next()?.parse().ok()?;
        if let Some(m) = parts.next() {
            mi = m.parse().ok()?;
        }
        if let Some(s) = parts.next() {
            sec = s.parse().ok()?;
        }
    }
    // Subtract the offset: a stamp of 01:03+00:00 is 01:03 UTC; 01:03-05:00 is 06:03 UTC.
    Some(unix_from_ymdhms(y, mo, d, h, mi, sec) - offset_s)
}

/// Parses a `±HH:MM` or `±HHMM` zone offset into seconds east of UTC.
fn parse_zone_offset(z: &str) -> Option<i64> {
    let b = z.as_bytes();
    let sign = match b.first()? {
        b'+' => 1i64,
        b'-' => -1i64,
        _ => return None,
    };
    let digits: String = z[1..].chars().filter(|c| *c != ':').collect();
    if digits.len() != 4 && digits.len() != 2 {
        return None;
    }
    let hh: i64 = digits[0..2].parse().ok()?;
    let mm: i64 = if digits.len() == 4 { digits[2..4].parse().ok()? } else { 0 };
    Some(sign * (hh * 3600 + mm * 60))
}

/// Parses any of the textual timestamp forms above into UTC unix seconds. Tries, in order: an
/// all-digits compact stamp, an SWPC `:Issued:`-style stamp (year + month-abbr), then the ISO
/// family. Returns `None` when nothing matches — never a guessed date.
#[must_use]
pub fn parse_timestamp(s: &str) -> Option<i64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    if t.bytes().all(|c| c.is_ascii_digit()) {
        if let Some(v) = parse_compact(t) {
            return Some(v);
        }
    }
    // "2026 Oct 06 0005 UTC" (and tolerant variants): second token is a month abbreviation.
    let toks: Vec<&str> = t.split_whitespace().collect();
    if toks.len() >= 4 && toks[0].len() == 4 && month_abbr(toks[1]).is_some() {
        if let Some(v) = parse_swpc_issued(t) {
            return Some(v);
        }
    }
    parse_iso(t)
}

/// Extracts the SWPC `:Issued:` stamp from a raw text bulletin (WWV / geomag forecast), returning
/// its unix seconds. `None` when the header is absent or unparseable — the caller falls back to the
/// fetch time rather than inventing an issue date.
#[must_use]
pub fn parse_issued_header(body: &str) -> Option<i64> {
    for line in body.lines().take(8) {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(":Issued:") {
            if let Some(t) = parse_timestamp(rest.trim()) {
                return Some(t);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    // One verified anchor (Python `calendar.timegm((2026,10,6,0,0,0))`); every other assertion is
    // an equality against a correctly-parsed reference form, so no second hand-computed constant can
    // silently drift.
    const ANCHOR_2026_10_06: i64 = 1_791_244_800;

    #[test]
    fn epoch_day_zero_is_1970() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(unix_from_ymdhms(1970, 1, 1, 0, 0, 0), 0);
    }

    #[test]
    fn the_anchor_matches_a_known_unix_time() {
        assert_eq!(parse_timestamp("2026-10-06T00:00:00Z"), Some(ANCHOR_2026_10_06));
        assert_eq!(parse_timestamp("2026-10-06T00:56:00Z"), Some(ANCHOR_2026_10_06 + 56 * 60));
        assert_eq!(parse_timestamp("2026-10-06T01:00:00Z"), Some(ANCHOR_2026_10_06 + 3600));
    }

    #[test]
    fn iso_variants_agree_with_the_canonical_z_form() {
        let z = parse_timestamp("2026-10-06T00:56:00Z");
        assert_eq!(parse_timestamp("2026-10-06T00:56:00"), z); // no zone => UTC
        assert_eq!(parse_timestamp("2026-10-06 00:56:00"), z); // space separator
        assert_eq!(parse_timestamp("2026-10-06T00:56"), z); // no seconds
        assert_eq!(parse_timestamp("2026-10-06T00:56:00.000Z"), z); // fractional dropped
        assert_eq!(
            parse_timestamp("2026-10-05T11:57:27.650880"),
            parse_timestamp("2026-10-05T11:57:27Z")
        );
    }

    #[test]
    fn offsets_are_applied_to_utc() {
        assert_eq!(
            parse_timestamp("2026-10-06T01:03:09+00:00"),
            parse_timestamp("2026-10-06T01:03:09Z")
        );
        assert_eq!(
            parse_timestamp("2026-10-06T01:00:00-05:00"),
            parse_timestamp("2026-10-06T06:00:00Z")
        );
        assert_eq!(
            parse_timestamp("2026-10-06T01:00:00+0000"),
            parse_timestamp("2026-10-06T01:00:00Z")
        );
    }

    #[test]
    fn compact_stamps_agree_with_iso() {
        assert_eq!(parse_timestamp("20261006"), parse_timestamp("2026-10-06T00:00:00Z"));
        assert_eq!(parse_timestamp("20260928005515"), parse_timestamp("2026-09-28T00:55:15Z"));
        assert_eq!(parse_timestamp("202610060056"), parse_timestamp("2026-10-06T00:56:00Z"));
    }

    #[test]
    fn swpc_issued_line_parses() {
        assert_eq!(parse_timestamp("2026 Oct 06 0005 UTC"), Some(ANCHOR_2026_10_06 + 5 * 60));
        let wwv =
            ":Product: Geophysical Alert Message wwv.txt\n:Issued: 2026 Oct 06 0005 UTC\n# body\n";
        assert_eq!(parse_issued_header(wwv), Some(ANCHOR_2026_10_06 + 5 * 60));
        assert_eq!(parse_issued_header("no header here"), None);
    }

    #[test]
    fn garbage_is_refused_not_guessed() {
        assert_eq!(parse_timestamp(""), None);
        assert_eq!(parse_timestamp("not a date"), None);
        assert_eq!(parse_timestamp("2026-13-45T99:99:99Z"), None); // month/day out of range
        assert_eq!(parse_timestamp("202610"), None); // 6 digits is not a compact form we accept
    }
}
