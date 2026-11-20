//! The local date, from the `Env`'s clock and its UTC offset (D1). Core reads no
//! clock or zone of its own (ADR-0003).

use crate::Env;

/// The proleptic Gregorian date of a day count from 1970-01-01 (Howard Hinnant's
/// `civil_from_days`). Shared with the changelog's timestamps.
pub fn civil(days: i64) -> (i64, u32, u32) {
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

/// The local date as `YYYY-MM-DD`: the clock's time plus its UTC offset, as Go's
/// `time.Now().Format("2006-01-02")` gives it.
pub fn today(env: &Env) -> String {
    let now = env.clock.now();
    let offset = i64::from(env.clock.utc_offset(now));
    // Whole seconds since the epoch, rounded down, so a time before it falls on
    // the day it's in.
    let secs = match now.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => {
            let d = e.duration();
            -(d.as_secs() as i64) - i64::from(d.subsec_nanos() > 0)
        }
    };
    let (y, m, d) = civil((secs + offset).div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::fake::FixedClock;

    fn today_at(t: SystemTime, offset: i32) -> String {
        let mut env = Env::fake(crate::env::Dirs::default());
        env.clock = Box::new(FixedClock::zoned(t, offset));
        today(&env)
    }

    /// 2001-02-03T23:30:00Z.
    fn late() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(981_243_000)
    }

    #[test]
    fn utc() {
        assert_eq!(today_at(late(), 0), "2001-02-03");
        assert_eq!(today_at(UNIX_EPOCH, 0), "1970-01-01");
    }

    #[test]
    fn east_of_utc_crosses_midnight() {
        assert_eq!(today_at(late(), 3600), "2001-02-04");
    }

    #[test]
    fn west_of_utc() {
        let early = late() - Duration::from_secs(23 * 3600); // 00:30Z
        assert_eq!(today_at(early, -3600), "2001-02-02");
        assert_eq!(today_at(early, 0), "2001-02-03");
    }

    #[test]
    fn before_the_epoch() {
        assert_eq!(
            today_at(UNIX_EPOCH - Duration::from_millis(1), 0),
            "1969-12-31"
        );
        assert_eq!(
            today_at(UNIX_EPOCH - Duration::from_secs(86_400), 0),
            "1969-12-31"
        );
        assert_eq!(
            today_at(UNIX_EPOCH - Duration::from_secs(86_401), 0),
            "1969-12-30"
        );
    }

    /// A plain `FixedClock` is UTC.
    #[test]
    fn fixed_clock_is_utc() {
        let mut env = Env::fake(crate::env::Dirs::default());
        env.clock = Box::new(FixedClock(late()));
        assert_eq!(today(&env), "2001-02-03");
    }
}
