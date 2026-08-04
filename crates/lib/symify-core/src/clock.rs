//! Time source, injected so the pure layers and tests stay deterministic.
//!
//! The only thing symify needs a clock for is naming backup files
//! (`<name>.<YYYYMMDDHHMMSS>.bak`). Keeping it behind a trait means tests can pin
//! the timestamp and assert exact `.bak` names, and no wall-clock call leaks into
//! planning.

use std::time::{SystemTime, UNIX_EPOCH};

/// Supplies the timestamp used in backup filenames.
pub trait Clock {
    /// A UTC timestamp formatted as `YYYYMMDDHHMMSS`.
    fn timestamp(&self) -> String;
}

/// Real clock backed by the system wall clock (UTC).
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn timestamp(&self) -> String {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        format_timestamp(secs)
    }
}

/// Fixed clock for tests; returns the same string every call.
#[derive(Debug, Clone)]
pub struct FixedClock(pub String);

impl Clock for FixedClock {
    fn timestamp(&self) -> String {
        self.0.clone()
    }
}

/// Format seconds-since-epoch (UTC) as `YYYYMMDDHHMMSS`.
fn format_timestamp(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (tod / 3600, (tod % 3600) / 60, tod % 60);
    format!("{y:04}{m:02}{d:02}{hh:02}{mm:02}{ss:02}")
}

/// Convert days-since-Unix-epoch to a `(year, month, day)` civil date (UTC).
///
/// Howard Hinnant's `civil_from_days` algorithm (public domain).
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_known_epochs() {
        // 2021-01-01T00:00:00Z = 1609459200
        assert_eq!(format_timestamp(1_609_459_200), "20210101000000");
        // 1970-01-01T00:00:00Z
        assert_eq!(format_timestamp(0), "19700101000000");
        // 2026-05-27T13:14:15Z = 1779887655
        assert_eq!(format_timestamp(1_779_887_655), "20260527131415");
    }

    #[test]
    fn fixed_clock_is_stable() {
        let c = FixedClock("20260101000000".into());
        assert_eq!(c.timestamp(), "20260101000000");
        assert_eq!(c.timestamp(), c.timestamp());
    }
}
