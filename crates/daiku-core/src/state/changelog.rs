//! `LOG.md`: one `- <UTC timestamp> <line>` entry per event, so logs merge cleanly
//! with git's union driver. A port of Go's `internal/core/changelog`, with the
//! time formatted as Go's `2006-01-02T15:04:05.000Z`.

use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{Error, gofmt, write};

/// `t` as `YYYY-MM-DDTHH:MM:SS.mmmZ` in UTC.
pub fn stamp(t: SystemTime) -> String {
    let (secs, millis) = match t.duration_since(UNIX_EPOCH) {
        Ok(d) => (d.as_secs() as i64, d.subsec_millis()),
        Err(e) => {
            // Before the epoch: Go truncates toward the earlier millisecond.
            let d = e.duration();
            let mut secs = -(d.as_secs() as i64);
            let nanos = d.subsec_nanos();
            let mut millis = 0;
            if nanos > 0 {
                secs -= 1;
                millis = (1_000_000_000 - nanos) / 1_000_000;
            }
            (secs, millis)
        }
    };
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// The proleptic Gregorian date of a day count from 1970-01-01 (Howard Hinnant's
/// `civil_from_days`).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// Go's `Append`: adds one entry to the log at `path` (under `root`, through the
/// write guard), creating it and its folder when absent.
pub fn append(root: &Path, path: &Path, at: SystemTime, line: &str) -> Result<(), Error> {
    write::check_path(root, path)?;
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        write::mkdir_all(dir)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .map_err(|source| Error::Io {
            op: "open",
            path: gofmt::to_slash(&path.to_string_lossy()),
            source,
        })?;
    f.write_all(format!("- {} {line}\n", stamp(at)).as_bytes())
        .map_err(|source| Error::Io {
            op: "write",
            path: gofmt::to_slash(&path.to_string_lossy()),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn stamps_in_go_layout() {
        assert_eq!(stamp(UNIX_EPOCH), "1970-01-01T00:00:00.000Z");
        let t = UNIX_EPOCH + Duration::from_millis(1_795_545_433_123);
        assert_eq!(stamp(t), "2026-11-24T18:37:13.123Z");
        assert_eq!(
            stamp(UNIX_EPOCH + Duration::from_secs(951_782_400)),
            "2000-02-29T00:00:00.000Z"
        );
        assert_eq!(
            stamp(UNIX_EPOCH - Duration::from_millis(1)),
            "1969-12-31T23:59:59.999Z"
        );
    }

    #[test]
    fn appends_lines() {
        let dir = crate::testutil::Temp::new();
        let log = dir.p("LOG.md");
        append(dir.path(), &log, UNIX_EPOCH, "a: created").unwrap();
        append(dir.path(), &log, UNIX_EPOCH, "a: intake → approved").unwrap();
        assert_eq!(
            std::fs::read_to_string(&log).unwrap(),
            "- 1970-01-01T00:00:00.000Z a: created\n- 1970-01-01T00:00:00.000Z a: intake → approved\n"
        );
    }
}
