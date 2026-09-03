use fasttime::{Date, DateTime, Time, Weekday};
use libm::floor;

use crate::SECONDS_PER_DAY;

pub const J2000: f64 = 2451545.0;
pub const J1970: f64 = 2440588.0;
pub const J1970_UNIX_EPOCH: f64 = J1970 - 0.5;
pub const DAYS_PER_JULIAN_CENTURY: f64 = 36525.0;

pub const D2000: Date = Date::from_ymd_unchecked(2000, 1, 1);
pub const D1970: Date = Date::from_ymd_unchecked(1970, 1, 1);
pub const MIDNIGHT: Time = Time {
    hour: 0,
    minute: 0,
    second: 0,
    nanosecond: 0,
};
pub const UNIX_EPOCH: DateTime = DateTime {
    date: D1970,
    time: MIDNIGHT,
};

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

    /// local sidereal time in degrees, assume the current datetime is in UT
    #[inline]
    fn to_sidereal_time(&self, lon: f64) -> f64 {
        let jd = self.to_julian_epoch_2000();
        sidereal_time(jd, lon)
    }

    /// local sidereal time in HMS, assume the current datetime is in UT
    fn to_sidereal_time_hms(&self, lon: f64) -> (u8, u8, u8) {
        let hr = self.to_sidereal_time(lon) / 15.0;
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
    /// default, date and time struct in fasttime crate
    /// does not have default value
    fn default() -> Self;

    /// year
    fn year(&self) -> i32;

    /// month
    fn month(&self) -> u8;

    /// day
    fn day(&self) -> u8;

    /// is the current year leap year
    fn is_leap_year(&self) -> bool {
        let year = self.year();
        let century_candidate = year % 25 == 0;
        (year & if century_candidate { 15 } else { 3 }) == 0
    }

    /// decimal year
    fn decimal_year(&self) -> f64 {
        self.decimal_year_with_offset_days(0.0)
    }

    /// decimal year by today with `offset` days
    fn decimal_year_with_offset_days(&self, offset: f64) -> f64;

    /// the day of n-th weekday of the current month, return `None` if it's outside this month
    fn nth_weekday(&self, weekday: Weekday, n: u8) -> Option<u8>;

    /// the day of first occurence weekday of the current month
    fn first_weekday(&self, weekday: Weekday) -> u8 {
        self.nth_weekday(weekday, 1).unwrap()
    }

    /// the day of the last occurence weekday of the current month
    fn last_weekday(&self, weekday: Weekday) -> u8;

    /// the date of n-th weekday of the current month, return `None` if it's outside this month
    fn nth_weekday_date(&self, weekday: Weekday, n: u8) -> Option<Date> {
        self.nth_weekday(weekday, n)
            .map(|d| Date::from_ymd_unchecked(self.year(), self.month(), d))
    }

    /// the date of first occurence weekday of the current month
    fn first_weekday_date(&self, weekday: Weekday) -> Date {
        Date::from_ymd_unchecked(self.year(), self.month(), self.first_weekday(weekday))
    }

    /// the date of the last occurence weekday of the current month
    fn last_weekday_date(&self, weekday: Weekday) -> Date {
        Date::from_ymd_unchecked(self.year(), self.month(), self.last_weekday(weekday))
    }

    /// how many days in the current year, 366 days if it's leap year, 365 days otherwise
    fn days_per_year(&self) -> u16 {
        if self.is_leap_year() { 366 } else { 365 }
    }

    /// how many days in the current month
    fn days_per_month(&self) -> u8 {
        // derived from a private function days_in_month() in fasttime crate
        let month = self.month();
        if month == 2 {
            if self.is_leap_year() { 29 } else { 28 }
        } else {
            30 | (month ^ (month >> 3))
        }
    }
}

impl AstronDatetimeExt for DateTime {
    fn from_julian(jd: f64) -> Self {
        let unix_secs = ((jd - J1970_UNIX_EPOCH) * SECONDS_PER_DAY) as i64;
        DateTime::from_unix_timestamp(unix_secs, 0).unwrap_or(DateTime::default())
    }

    fn to_julian(&self) -> f64 {
        self.unix_timestamp() as f64 / SECONDS_PER_DAY + J1970_UNIX_EPOCH
    }
}

impl AstronDatetimeExt for Date {
    fn from_julian(jd: f64) -> Self {
        Date::from_days_since_unix_epoch((jd - J1970_UNIX_EPOCH) as i64).unwrap()
    }

    fn to_julian(&self) -> f64 {
        self.days_since_unix_epoch() as f64 + J1970_UNIX_EPOCH
    }
}

impl DateExt for DateTime {
    fn decimal_year_with_offset_days(&self, offset: f64) -> f64 {
        self.date.decimal_year_with_offset_days(offset)
    }

    fn nth_weekday(&self, weekday: Weekday, n: u8) -> Option<u8> {
        self.date.nth_weekday(weekday, n)
    }

    fn last_weekday(&self, weekday: Weekday) -> u8 {
        self.date.last_weekday(weekday)
    }

    #[inline]
    fn year(&self) -> i32 {
        self.date.year()
    }

    #[inline]
    fn month(&self) -> u8 {
        self.date.month()
    }

    #[inline]
    fn day(&self) -> u8 {
        self.date.day()
    }

    fn default() -> Self {
        UNIX_EPOCH
    }
}

impl DateExt for Date {
    fn decimal_year_with_offset_days(&self, offset: f64) -> f64 {
        (self.ordinal() as f64 + offset) / self.days_per_year() as f64 + self.year as f64
    }

    /// derived from https://rosettacode.org/wiki/Nth_Particular_Weekday_of_the_Month
    fn nth_weekday(&self, weekday: Weekday, n: u8) -> Option<u8> {
        let first_weekday = Date::from_ymd_unchecked(self.year, self.month, 1)
            .weekday()
            .number_from_monday() as i8;
        let weekday = weekday.number_from_monday() as i8;

        // 1 + first occurence + n-th occurence
        // let target_day = 1 + (weekday - first_weekday) + (n as i8 - 1) * 7;
        let target_day =
            1 + (weekday - first_weekday) % 7 + 7 * (n as i8 - (first_weekday <= weekday) as i8);
        if target_day as u8 > self.days_per_month() {
            None
        } else {
            Some(target_day as u8)
        }
    }

    fn last_weekday(&self, weekday: Weekday) -> u8 {
        let last_date = Date::from_ymd_unchecked(self.year, self.month, self.days_per_month());
        let last_weekday = last_date.weekday().number_from_monday() as i8;
        let weekday = weekday.number_from_monday() as i8;

        let days_diff = (weekday - last_weekday) % 7;
        let days_back = if days_diff < 0 { days_diff } else { 7 } - days_diff;

        (last_date.day as i8 - days_back) as u8
    }

    #[inline]
    fn year(&self) -> i32 {
        self.year
    }

    #[inline]
    fn month(&self) -> u8 {
        self.month
    }

    #[inline]
    fn day(&self) -> u8 {
        self.day
    }

    fn default() -> Self {
        D1970
    }
}

/// Espenak & Meeus polynomial of delta T for 2005 to 2050,
/// return delta T in days.
/// `y`: decimal year between 2005.0 to 2050.0
pub fn delta_t_2000(y: f64) -> f64 {
    let t = y - 2000.0;

    (62.92 + 0.32217 * t + 0.005589 * t * t) / SECONDS_PER_DAY
}

/// local sidereal time in degrees, `jd2000` is the julian day epoch since J2000
pub fn sidereal_time(jd2000: f64, lon: f64) -> f64 {
    let t = jd2000 / DAYS_PER_JULIAN_CENTURY;
    let gmst =
        280.46061837 + 360.98564736629 * jd2000 + 0.000387933 * t * t - (t * t * t) / 38710000.0;

    let lst = (gmst + lon) % 360.0;
    if lst < 0.0 { lst + 360.0 } else { lst }
}
