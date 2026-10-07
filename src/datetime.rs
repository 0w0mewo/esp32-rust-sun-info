use core::f64::consts::TAU;

use libm::floor;
use time::{Date, Duration, OffsetDateTime, PlainDateTime, Timestamp, UtcDateTime, UtcOffset};

use crate::{
    HOUR_PER_RAD, SECONDS_PER_DAY,
    config::{TZ_DST_RULES, TZ_DST_RULES_START_YEAR},
};

pub const J2000: f64 = 2451545.0;
pub const J1970: f64 = 2440588.0;
pub const J1970_UNIX_EPOCH: f64 = J1970 - 0.5;
pub const DAYS_PER_JULIAN_CENTURY: f64 = 36525.0;
pub const DAY_PER_HOUR: f64 = 1.0 / 24.0;
pub const UNIX_EPOCH_PLAIN: PlainDateTime = PlainDateTime::new(
    UtcDateTime::UNIX_EPOCH.date(),
    UtcDateTime::UNIX_EPOCH.time(),
);

pub trait AstronDatetimeExt: DateExt {
    /// convert from julian days to civil datetime
    /// Note: f64 used here because f32 was not precise enough
    fn from_julian(jd: f64) -> Self;

    /// convert to julian days
    fn to_julian(&self) -> f64;

    /// convert to julian days epoch since J2000
    fn to_julian_epoch_2000(&self) -> f64 {
        self.to_julian() - J2000
    }

    /// convert to julian centuries
    fn to_julian_centuries(&self) -> f64 {
        self.to_julian() / DAYS_PER_JULIAN_CENTURY
    }

    /// local sidereal time in radians, assume the current datetime is in UT
    #[inline]
    fn to_sidereal_time(&self, lon_rad: f64) -> f64 {
        let jd = self.to_julian();
        sidereal_time(jd, lon_rad)
    }

    /// local sidereal time in HMS, assume the current datetime is in UT
    fn to_sidereal_time_hms(&self, lon_rad: f64) -> (u8, u8, u8) {
        let hr = self.to_sidereal_time(lon_rad) * HOUR_PER_RAD;
        let h = floor(hr);
        let m_decimal = (hr - h) * 60.0;
        let m = floor(m_decimal);
        let s = (m_decimal - m) * 60.0;

        (h as u8, m as u8, s as u8)
    }

    /// delta T in days
    fn delta_t(&self) -> f64 {
        delta_t_2000(self.decimal_year())
    }
}

pub trait DateExt {
    // current year
    fn cur_year(&self) -> i32;
    // ordinal day of year
    fn ordinal_day(&self) -> u16;
    /// decimal year
    fn decimal_year(&self) -> f64 {
        self.decimal_year_with_offset_days(0.0)
    }
    /// decimal year by today with `offset` days
    fn decimal_year_with_offset_days(&self, offset: f64) -> f64 {
        let days_per_year = if self.is_leap_year() { 366.0 } else { 365.0 };
        (self.ordinal_day() as f64 + offset) / days_per_year + self.cur_year() as f64
    }
    /// is leap year
    fn is_leap_year(&self) -> bool {
        time::util::is_leap_year(self.cur_year())
    }
}

impl AstronDatetimeExt for PlainDateTime {
    fn from_julian(jd: f64) -> Self {
        let unix_secs = ((jd - J1970_UNIX_EPOCH) * SECONDS_PER_DAY) as i64;
        let ts_now = Timestamp::from_seconds(unix_secs).unwrap_or(Timestamp::UNIX_EPOCH);
        Self::new(ts_now.date(), ts_now.time())
    }

    fn to_julian(&self) -> f64 {
        self.as_utc().to_julian()
    }
}

impl AstronDatetimeExt for UtcDateTime {
    fn from_julian(jd: f64) -> Self {
        let unix_secs = ((jd - J1970_UNIX_EPOCH) * SECONDS_PER_DAY) as i64;
        UtcDateTime::from_unix_timestamp(unix_secs).unwrap_or(UtcDateTime::UNIX_EPOCH)
    }

    fn to_julian(&self) -> f64 {
        self.unix_timestamp() as f64 / SECONDS_PER_DAY + J1970_UNIX_EPOCH
    }
}

impl AstronDatetimeExt for Date {
    fn from_julian(jd: f64) -> Self {
        Date::from_julian_day((jd - J1970_UNIX_EPOCH) as i32).unwrap()
    }

    fn to_julian(&self) -> f64 {
        self.to_julian_day() as f64 - 0.5
    }
}

impl AstronDatetimeExt for OffsetDateTime {
    /// convert from JD, the resulting datetime is in UTC
    fn from_julian(jd: f64) -> Self {
        UtcDateTime::from_julian(jd).to_offset(UtcOffset::UTC)
    }

    /// convert to JD in UTC
    fn to_julian(&self) -> f64 {
        self.to_utc().to_julian()
    }
}

impl DateExt for UtcDateTime {
    #[inline]
    fn cur_year(&self) -> i32 {
        self.year()
    }

    #[inline]
    fn ordinal_day(&self) -> u16 {
        self.ordinal()
    }
}

impl DateExt for Date {
    #[inline]
    fn cur_year(&self) -> i32 {
        self.year()
    }

    #[inline]
    fn ordinal_day(&self) -> u16 {
        self.ordinal()
    }
}

impl DateExt for PlainDateTime {
    #[inline]
    fn cur_year(&self) -> i32 {
        self.year()
    }

    #[inline]
    fn ordinal_day(&self) -> u16 {
        self.ordinal()
    }
}

impl DateExt for OffsetDateTime {
    #[inline]
    fn cur_year(&self) -> i32 {
        self.year()
    }

    #[inline]
    fn ordinal_day(&self) -> u16 {
        self.ordinal()
    }
}

pub trait UtOffsetExt {
    /// if the current datetime inside DST range
    fn is_dst(&self) -> bool;
    /// convert to `OffsetDateTime` with DST and standard time zone offset applied
    fn with_dst_offset(&self, tz_offset: &UtcOffset) -> OffsetDateTime;

    /// add 1 hour if the current datetime is inside DST range, 0 if not
    fn dst_offset_duration(&self) -> Duration {
        let hr = if self.is_dst() { 1 } else { 0 };
        Duration::hours(hr)
    }

    /// convert to `PlainDatetime` with DST and standard time zone offset applied
    fn with_dst_offset_plain(&self, tz_offset: &UtcOffset) -> PlainDateTime {
        let offset_datetime = self.with_dst_offset(tz_offset);
        PlainDateTime::new(offset_datetime.date(), offset_datetime.time())
    }
}

impl UtOffsetExt for OffsetDateTime {
    fn is_dst(&self) -> bool {
        self.to_utc().is_dst()
    }

    /// apply DST offset to offsetted datetime, the parameter, `tz_offset` is unused,
    /// use `time::UtcOffset::UTC` as placeholder
    fn with_dst_offset(&self, _tz_offset: &UtcOffset) -> OffsetDateTime {
        self.saturating_add(self.dst_offset_duration())
    }
}

impl UtOffsetExt for UtcDateTime {
    fn is_dst(&self) -> bool {
        if TZ_DST_RULES.is_empty() {
            return false;
        }

        // get the rule for current year from LUT
        let tz_dst_lut_idx = self.year() as usize - TZ_DST_RULES_START_YEAR;
        TZ_DST_RULES.get(tz_dst_lut_idx).is_some_and(|tz_dst_rule| {
            let unix_sec = self.unix_timestamp();
            let dst_start = tz_dst_rule.0;
            let dst_end = tz_dst_rule.1;

            if dst_end < dst_start {
                // some zones in the southern hemisphere, like Australia
                !(dst_end..=dst_start).contains(&unix_sec)
            } else {
                (dst_start..=dst_end).contains(&unix_sec)
            }
        })
    }

    fn with_dst_offset(&self, tz_offset: &UtcOffset) -> OffsetDateTime {
        self.to_offset(*tz_offset)
            .saturating_add(self.dst_offset_duration())
    }
}

impl UtOffsetExt for PlainDateTime {
    fn is_dst(&self) -> bool {
        self.as_utc().is_dst()
    }

    fn with_dst_offset(&self, tz_offset: &UtcOffset) -> OffsetDateTime {
        self.as_utc().with_dst_offset(tz_offset)
    }
}

/// Espenak & Meeus polynomial of delta T for 2005 to 2050,
/// return delta T in days.
/// `y`: decimal year between 2000.0 to 3000.0
/// https://www.eclipsewise.com/help/deltatpoly2014.html
pub fn delta_t_2000(y: f64) -> f64 {
    let dt_sec = if y < 2005.0 {
        // 1986 to 2005
        let t = y - 2000.0;
        let t2 = t * t;
        let t3 = t2 * t;
        let t4 = t3 * t;
        let t5 = t4 * t;

        63.86 + 0.3345 * t - 0.060374 * t2 + 0.0017275 * t3 + 0.000651814 * t4 + 0.00002373599 * t5
    } else if y < 2015.0 {
        // 2005 to 2015
        let t = y - 2005.0;
        64.69 + 0.2930 * t
    } else {
        // 2015 to 3000
        let t = y - 2015.0;
        67.62 + 0.3645 * t + 0.0039755 * t * t
    };

    dt_sec / SECONDS_PER_DAY
}

/// local sidereal time in radians
pub fn sidereal_time(jd: f64, lon_rad: f64) -> f64 {
    let jd2000 = jd - J2000;
    let t = jd2000 / DAYS_PER_JULIAN_CENTURY;

    let gmst =
        280.46061837 + 360.98564736629 * jd2000 + 0.000387933 * t * t - (t * t * t) / 38710000.0;
    let gmst_rad = gmst.to_radians();

    let lst = (gmst_rad + lon_rad) % TAU;
    if lst < 0.0 { lst + TAU } else { lst }
}
